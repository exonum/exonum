//  Copyright (c) 2011-present, Facebook, Inc.  All rights reserved.
//  This source code is licensed under the BSD-style license found in the
//  LICENSE file in the root directory of this source tree. An additional grant
//  of patent rights can be found in the PATENTS file in the same directory.

#ifndef ROCKSDB_LITE

#ifndef __STDC_FORMAT_MACROS
#define __STDC_FORMAT_MACROS
#endif

#include "utilities/transactions/transaction_lock_mgr.h"

#include <inttypes.h>

#include <algorithm>
#include <condition_variable>
#include <functional>
#include <mutex>
#include <string>
#include <vector>

#include "rocksdb/slice.h"
#include "rocksdb/utilities/transaction_db_mutex.h"
#include "util/autovector.h"
#include "util/murmurhash.h"
#include "util/sync_point.h"
#include "util/thread_local.h"
#include "utilities/transactions/transaction_db_impl.h"

namespace rocksdb {

struct LockInfo {
  TransactionID txn_id;

  // Transaction locks are not valid after this time in us
  uint64_t expiration_time;

  LockInfo(TransactionID id, uint64_t time)
      : txn_id(id), expiration_time(time) {}
  LockInfo(const LockInfo& lock_info)
      : txn_id(lock_info.txn_id), expiration_time(lock_info.expiration_time) {}
};

struct LockMapStripe {
  explicit LockMapStripe(std::shared_ptr<TransactionDBMutexFactory> factory) {
    stripe_mutex = factory->AllocateMutex();
    stripe_cv = factory->AllocateCondVar();
    assert(stripe_mutex);
    assert(stripe_cv);
  }

  // Mutex must be held before modifying keys map
  std::shared_ptr<TransactionDBMutex> stripe_mutex;

  // Condition Variable per stripe for waiting on a lock
  std::shared_ptr<TransactionDBCondVar> stripe_cv;

  // Locked keys mapped to the info about the transactions that locked them.
  // TODO(agiardullo): Explore performance of other data structures.
  std::unordered_map<std::string, LockInfo> keys;
};

// Map of #num_stripes LockMapStripes
struct LockMap {
  explicit LockMap(size_t num_stripes,
                   std::shared_ptr<TransactionDBMutexFactory> factory)
      : num_stripes_(num_stripes) {
    lock_map_stripes_.reserve(num_stripes);
    for (size_t i = 0; i < num_stripes; i++) {
      LockMapStripe* stripe = new LockMapStripe(factory);
      lock_map_stripes_.push_back(stripe);
    }
  }

  ~LockMap() {
    for (auto stripe : lock_map_stripes_) {
      delete stripe;
    }
  }

  // Number of sepearate LockMapStripes to create, each with their own Mutex
  const size_t num_stripes_;

  // Count of keys that are currently locked in this column family.
  // (Only maintained if TransactionLockMgr::max_num_locks_ is positive.)
  std::atomic<int64_t> lock_cnt{0};

  std::vector<LockMapStripe*> lock_map_stripes_;

  size_t GetStripe(const std::string& key) const;
};

namespace {
void UnrefLockMapsCache(void* ptr) {
  // Called when a thread exits or a ThreadLocalPtr gets destroyed.
  auto lock_maps_cache =
      static_cast<std::unordered_map<uint32_t, std::shared_ptr<LockMap>>*>(ptr);
  delete lock_maps_cache;
}
}  // anonymous namespace

TransactionLockMgr::TransactionLockMgr(
    TransactionDB* txn_db, size_t default_num_stripes, int64_t max_num_locks,
    std::shared_ptr<TransactionDBMutexFactory> mutex_factory)
    : txn_db_impl_(nullptr),
      default_num_stripes_(default_num_stripes),
      max_num_locks_(max_num_locks),
      lock_maps_cache_(new ThreadLocalPtr(&UnrefLockMapsCache)),
      mutex_factory_(mutex_factory) {
  txn_db_impl_ = dynamic_cast<TransactionDBImpl*>(txn_db);
  assert(txn_db_impl_);
}

TransactionLockMgr::~TransactionLockMgr() {}

size_t LockMap::GetStripe(const std::string& key) const {
  assert(num_stripes_ > 0);
  static murmur_hash hash;
  size_t stripe = hash(key) % num_stripes_;
  return stripe;
}

void TransactionLockMgr::AddColumnFamily(uint32_t column_family_id) {
  InstrumentedMutexLock l(&lock_map_mutex_);

  if (lock_maps_.find(column_family_id) == lock_maps_.end()) {
    lock_maps_.emplace(column_family_id,
                       std::shared_ptr<LockMap>(
                           new LockMap(default_num_stripes_, mutex_factory_)));
  } else {
    // column_family already exists in lock map
    assert(false);
  }
}

void TransactionLockMgr::RemoveColumnFamily(uint32_t column_family_id) {
  // Remove lock_map for this column family.  Since the lock map is stored
  // as a shared ptr, concurrent transactions can still keep using it
  // until they release their references to it.
  {
    InstrumentedMutexLock l(&lock_map_mutex_);

    auto lock_maps_iter = lock_maps_.find(column_family_id);
    assert(lock_maps_iter != lock_maps_.end());

    lock_maps_.erase(lock_maps_iter);
  }  // lock_map_mutex_

  // Clear all thread-local caches
  autovector<void*> local_caches;
  lock_maps_cache_->Scrape(&local_caches, nullptr);
  for (auto cache : local_caches) {
    delete static_cast<LockMaps*>(cache);
  }
}

// Look up the LockMap shared_ptr for a given column_family_id.
// Note:  The LockMap is only valid as long as the caller is still holding on
//   to the returned shared_ptr.
std::shared_ptr<LockMap> TransactionLockMgr::GetLockMap(
    uint32_t column_family_id) {
  // First check thread-local cache
  if (lock_maps_cache_->Get() == nullptr) {
    lock_maps_cache_->Reset(new LockMaps());
  }

  auto lock_maps_cache = static_cast<LockMaps*>(lock_maps_cache_->Get());

  auto lock_map_iter = lock_maps_cache->find(column_family_id);
  if (lock_map_iter != lock_maps_cache->end()) {
    // Found lock map for this column family.
    return lock_map_iter->second;
  }

  // Not found in local cache, grab mutex and check shared LockMaps
  InstrumentedMutexLock l(&lock_map_mutex_);

  lock_map_iter = lock_maps_.find(column_family_id);
  if (lock_map_iter == lock_maps_.end()) {
    return std::shared_ptr<LockMap>(nullptr);
  } else {
    // Found lock map.  Store in thread-local cache and return.
    std::shared_ptr<LockMap>& lock_map = lock_map_iter->second;
    lock_maps_cache->insert({column_family_id, lock_map});

    return lock_map;
  }
}

// Returns true if this lock has expired and can be acquired by another
// transaction.
// If false, sets *expire_time to the expiration time of the lock according
// to Env->GetMicros() or 0 if no expiration.
bool TransactionLockMgr::IsLockExpired(const LockInfo& lock_info, Env* env,
                                       uint64_t* expire_time) {
  auto now = env->NowMicros();

  bool expired =
      (lock_info.expiration_time > 0 && lock_info.expiration_time <= now);

  if (!expired && lock_info.expiration_time > 0) {
    // return how many microseconds until lock will be expired
    *expire_time = lock_info.expiration_time;
  } else {
    bool success =
        txn_db_impl_->TryStealingExpiredTransactionLocks(lock_info.txn_id);
    if (!success) {
      expired = false;
    }
    *expire_time = 0;
  }

  return expired;
}

Status TransactionLockMgr::TryLock(TransactionImpl* txn,
                                   uint32_t column_family_id,
                                   const std::string& key, Env* env) {
  // Lookup lock map for this column family id
  std::shared_ptr<LockMap> lock_map_ptr = GetLockMap(column_family_id);
  LockMap* lock_map = lock_map_ptr.get();
  if (lock_map == nullptr) {
    char msg[255];
    snprintf(msg, sizeof(msg), "Column family id not found: %" PRIu32,
             column_family_id);

    return Status::InvalidArgument(msg);
  }

  // Need to lock the mutex for the stripe that this key hashes to
  size_t stripe_num = lock_map->GetStripe(key);
  assert(lock_map->lock_map_stripes_.size() > stripe_num);
  LockMapStripe* stripe = lock_map->lock_map_stripes_.at(stripe_num);

  LockInfo lock_info(txn->GetID(), txn->GetExpirationTime());
  int64_t timeout = txn->GetLockTimeout();

  return AcquireWithTimeout(txn, lock_map, stripe, column_family_id, key, env,
                            timeout, lock_info);
}

// Helper function for TryLock().
Status TransactionLockMgr::AcquireWithTimeout(
    TransactionImpl* txn, LockMap* lock_map, LockMapStripe* stripe,
    uint32_t column_family_id, const std::string& key, Env* env,
    int64_t timeout, const LockInfo& lock_info) {
  Status result;
  uint64_t start_time = 0;
  uint64_t end_time = 0;

  if (timeout > 0) {
    start_time = env->NowMicros();
    end_time = start_time + timeout;
  }

  if (timeout < 0) {
    // If timeout is negative, we wait indefinitely to acquire the lock
    result = stripe->stripe_mutex->Lock();
  } else {
    result = stripe->stripe_mutex->TryLockFor(timeout);
  }

  if (!result.ok()) {
    // failed to acquire mutex
    return result;
  }

  // Acquire lock if we are able to
  uint64_t expire_time_hint = 0;
  TransactionID wait_id = 0;
  result = AcquireLocked(lock_map, stripe, key, env, lock_info,
                         &expire_time_hint, &wait_id);

  if (!result.ok() && timeout != 0) {
    // If we weren't able to acquire the lock, we will keep retrying as long
    // as the timeout allows.
    bool timed_out = false;
    do {
      // Decide how long to wait
      int64_t cv_end_time = -1;

      // Check if held lock's expiration time is sooner than our timeout
      if (expire_time_hint > 0 &&
          (timeout < 0 || (timeout > 0 && expire_time_hint < end_time))) {
        // expiration time is sooner than our timeout
        cv_end_time = expire_time_hint;
      } else if (timeout >= 0) {
        cv_end_time = end_time;
      }

      assert(result.IsBusy() || wait_id != 0);

      // We are dependent on a transaction to finish, so perform deadlock
      // detection.
      if (wait_id != 0) {
        if (txn->IsDeadlockDetect()) {
          if (IncrementWaiters(txn, wait_id)) {
            result = Status::Busy(Status::SubCode::kDeadlock);
            stripe->stripe_mutex->UnLock();
            return result;
          }
        }
        txn->SetWaitingTxn(wait_id, column_family_id, &key);
      }

      TEST_SYNC_POINT("TransactionLockMgr::AcquireWithTimeout:WaitingTxn");
      if (cv_end_time < 0) {
        // Wait indefinitely
        result = stripe->stripe_cv->Wait(stripe->stripe_mutex);
      } else {
        uint64_t now = env->NowMicros();
        if (static_cast<uint64_t>(cv_end_time) > now) {
          result = stripe->stripe_cv->WaitFor(stripe->stripe_mutex,
                                              cv_end_time - now);
        }
      }

      if (wait_id != 0) {
        txn->SetWaitingTxn(0, 0, nullptr);
        if (txn->IsDeadlockDetect()) {
          DecrementWaiters(txn, wait_id);
        }
      }

      if (result.IsTimedOut()) {
          timed_out = true;
          // Even though we timed out, we will still make one more attempt to
          // acquire lock below (it is possible the lock expired and we
          // were never signaled).
      }

      if (result.ok() || result.IsTimedOut()) {
        result = AcquireLocked(lock_map, stripe, key, env, lock_info,
                               &expire_time_hint, &wait_id);
      }
    } while (!result.ok() && !timed_out);
  }

  stripe->stripe_mutex->UnLock();

  return result;
}

void TransactionLockMgr::DecrementWaiters(const TransactionImpl* txn,
                                          TransactionID wait_id) {
  std::lock_guard<std::mutex> lock(wait_txn_map_mutex_);
  DecrementWaitersImpl(txn, wait_id);
}

void TransactionLockMgr::DecrementWaitersImpl(const TransactionImpl* txn,
                                              TransactionID wait_id) {
  auto id = txn->GetID();
  assert(wait_txn_map_.count(id) > 0);
  wait_txn_map_.erase(id);

  rev_wait_txn_map_[wait_id]--;
  if (rev_wait_txn_map_[wait_id] == 0) {
    rev_wait_txn_map_.erase(wait_id);
  }
}

bool TransactionLockMgr::IncrementWaiters(const TransactionImpl* txn,
                                          TransactionID wait_id) {
  auto id = txn->GetID();
  std::lock_guard<std::mutex> lock(wait_txn_map_mutex_);
  assert(wait_txn_map_.count(id) == 0);
  wait_txn_map_[id] = wait_id;
  rev_wait_txn_map_[wait_id]++;

  // No deadlock if nobody is waiting on self.
  if (rev_wait_txn_map_.count(id) == 0) {
    return false;
  }

  TransactionID next = wait_id;
  for (int i = 0; i < txn->GetDeadlockDetectDepth(); i++) {
    if (next == id) {
      DecrementWaitersImpl(txn, wait_id);
      return true;
    } else if (wait_txn_map_.count(next) == 0) {
      return false;
    } else {
      next = wait_txn_map_[next];
    }
  }

  // Wait cycle too big, just assume deadlock.
  DecrementWaitersImpl(txn, wait_id);
  return true;
}

// Try to lock this key after we have acquired the mutex.
// Sets *expire_time to the expiration time in microseconds
//  or 0 if no expiration.
// REQUIRED:  Stripe mutex must be held.
Status TransactionLockMgr::AcquireLocked(LockMap* lock_map,
                                         LockMapStripe* stripe,
                                         const std::string& key, Env* env,
                                         const LockInfo& txn_lock_info,
                                         uint64_t* expire_time,
                                         TransactionID* txn_id) {
  Status result;
  // Check if this key is already locked
  if (stripe->keys.find(key) != stripe->keys.end()) {
    // Lock already held

    LockInfo& lock_info = stripe->keys.at(key);
    if (lock_info.txn_id != txn_lock_info.txn_id) {
      // locked by another txn.  Check if it's expired
      if (IsLockExpired(lock_info, env, expire_time)) {
        // lock is expired, can steal it
        lock_info.txn_id = txn_lock_info.txn_id;
        lock_info.expiration_time = txn_lock_info.expiration_time;
        // lock_cnt does not change
      } else {
        result = Status::TimedOut(Status::SubCode::kLockTimeout);
        *txn_id = lock_info.txn_id;
      }
    }
  } else {  // Lock not held.
    // Check lock limit
    if (max_num_locks_ > 0 &&
        lock_map->lock_cnt.load(std::memory_order_acquire) >= max_num_locks_) {
      result = Status::Busy(Status::SubCode::kLockLimit);
    } else {
      // acquire lock
      stripe->keys.insert({key, txn_lock_info});

      // Maintain lock count if there is a limit on the number of locks
      if (max_num_locks_) {
        lock_map->lock_cnt++;
      }
    }
  }

  return result;
}

void TransactionLockMgr::UnLock(TransactionImpl* txn, uint32_t column_family_id,
                                const std::string& key, Env* env) {
  std::shared_ptr<LockMap> lock_map_ptr = GetLockMap(column_family_id);
  LockMap* lock_map = lock_map_ptr.get();
  if (lock_map == nullptr) {
    // Column Family must have been dropped.
    return;
  }

  // Lock the mutex for the stripe that this key hashes to
  size_t stripe_num = lock_map->GetStripe(key);
  assert(lock_map->lock_map_stripes_.size() > stripe_num);
  LockMapStripe* stripe = lock_map->lock_map_stripes_.at(stripe_num);

  TransactionID txn_id = txn->GetID();

  stripe->stripe_mutex->Lock();

  const auto& iter = stripe->keys.find(key);
  if (iter != stripe->keys.end() && iter->second.txn_id == txn_id) {
    // Found the key we locked.  unlock it.
    stripe->keys.erase(iter);
    if (max_num_locks_ > 0) {
      // Maintain lock count if there is a limit on the number of locks.
      assert(lock_map->lock_cnt.load(std::memory_order_relaxed) > 0);
      lock_map->lock_cnt--;
    }
  } else {
    // This key is either not locked or locked by someone else.  This should
    // only happen if the unlocking transaction has expired.
    assert(txn->GetExpirationTime() > 0 &&
           txn->GetExpirationTime() < env->NowMicros());
  }

  stripe->stripe_mutex->UnLock();

  // Signal waiting threads to retry locking
  stripe->stripe_cv->NotifyAll();
}

void TransactionLockMgr::UnLock(const TransactionImpl* txn,
                                const TransactionKeyMap* key_map, Env* env) {
  TransactionID txn_id = txn->GetID();

  for (auto& key_map_iter : *key_map) {
    uint32_t column_family_id = key_map_iter.first;
    auto& keys = key_map_iter.second;

    std::shared_ptr<LockMap> lock_map_ptr = GetLockMap(column_family_id);
    LockMap* lock_map = lock_map_ptr.get();

    if (lock_map == nullptr) {
      // Column Family must have been dropped.
      return;
    }

    // Bucket keys by lock_map_ stripe
    std::unordered_map<size_t, std::vector<const std::string*>> keys_by_stripe(
        std::max(keys.size(), lock_map->num_stripes_));

    for (auto& key_iter : keys) {
      const std::string& key = key_iter.first;

      size_t stripe_num = lock_map->GetStripe(key);
      keys_by_stripe[stripe_num].push_back(&key);
    }

    // For each stripe, grab the stripe mutex and unlock all keys in this stripe
    for (auto& stripe_iter : keys_by_stripe) {
      size_t stripe_num = stripe_iter.first;
      auto& stripe_keys = stripe_iter.second;

      assert(lock_map->lock_map_stripes_.size() > stripe_num);
      LockMapStripe* stripe = lock_map->lock_map_stripes_.at(stripe_num);

      stripe->stripe_mutex->Lock();

      for (const std::string* key : stripe_keys) {
        const auto& iter = stripe->keys.find(*key);
        if (iter != stripe->keys.end() && iter->second.txn_id == txn_id) {
          // Found the key we locked.  unlock it.
          stripe->keys.erase(iter);
          if (max_num_locks_ > 0) {
            // Maintain lock count if there is a limit on the number of locks.
            assert(lock_map->lock_cnt.load(std::memory_order_relaxed) > 0);
            lock_map->lock_cnt--;
          }
        } else {
          // This key is either not locked or locked by someone else.  This
          // should only
          // happen if the unlocking transaction has expired.
          assert(txn->GetExpirationTime() > 0 &&
                 txn->GetExpirationTime() < env->NowMicros());
        }
      }

      stripe->stripe_mutex->UnLock();

      // Signal waiting threads to retry locking
      stripe->stripe_cv->NotifyAll();
    }
  }
}

TransactionLockMgr::LockStatusData TransactionLockMgr::GetLockStatusData() {
  LockStatusData data;
  // Lock order here is important. The correct order is lock_map_mutex_, then
  // for every column family ID in ascending order lock every stripe in
  // ascending order.
  InstrumentedMutexLock l(&lock_map_mutex_);

  std::vector<uint32_t> cf_ids;
  for (const auto& map : lock_maps_) {
    cf_ids.push_back(map.first);
  }
  std::sort(cf_ids.begin(), cf_ids.end());

  for (auto i : cf_ids) {
    const auto& stripes = lock_maps_[i]->lock_map_stripes_;
    // Iterate and lock all stripes in ascending order.
    for (const auto& j : stripes) {
      j->stripe_mutex->Lock();
      for (const auto& it : j->keys) {
        data.insert({i, {it.first, it.second.txn_id}});
      }
    }
  }

  // Unlock everything. Unlocking order is not important.
  for (auto i : cf_ids) {
    const auto& stripes = lock_maps_[i]->lock_map_stripes_;
    for (const auto& j : stripes) {
      j->stripe_mutex->UnLock();
    }
  }

  return data;
}

}  //  namespace rocksdb
#endif  // ROCKSDB_LITE
