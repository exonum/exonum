// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// extern crate exonum;
// extern crate sandbox;

// use exonum::messages::{Message, Propose, Prevote, Precommit};

// use sandbox::timestamping_sandbox;


// =======================

// HANDLE REQUEST

// - ignore if to incorrect
// - ignore if incorrect time
// - ignore if time < 0
// - ignore if time > REQUEST_ALIVE
// - ignore if incorrect signature

// REQUEST PROPOSE:
// - ignore if wrong height
// - ignore if hasn’t propose
// - send propose

// REQUEST TXS:
// - ignore if hasn’t
// - send from pool
// - send from blockchain

// REQUEST PREVOTES:
// - ignore if height != our height
// - send prevotes we have (> +2/3, <+2/3, 0)

// REQUEST PRECOMITS:
// - ignore if height > our height
// - send precommits we have (> +2/3, <+2/3, 0) for out height
// - send precommits from blockchain for prev height if we have (or not send if haven’t)

// REQUEST COMMIT:
// - ignore if height = our height
// - ignore if height > our height
// - send +2/3 precommits if we have (not send if haven’t)

// BYZANTINE:
// - get precommits with different block_hash
// - send different proposes
// - not send proposes
// - update lock
