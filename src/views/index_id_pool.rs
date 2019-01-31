// Copyright 2019 The Exonum Team
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

use crate::Fork;

use super::{IndexAccess, IndexAddress, View};

const INDEX_ID_POOL_NAME: &str = "__INDEX_ID_POOL__";
const INDEX_ID_POOL_LEN: &str = "__INDEX_ID_POOL_LEN__";

/// TODO Add documentation. [ECR-2820]
struct IndexIdPool<T: IndexAccess> {
    view: View<T>,
}

impl IndexAddress {
    /// TODO Add documentation. [ECR-2820]
    fn to_path(&self) -> String {
        if let Some(bytes) = self.bytes.as_ref() {
            format!("{}/{}", self.name, hex::encode(bytes))
        } else {
            self.name.to_owned()
        }
    }
}

impl<T: IndexAccess> IndexIdPool<T> {
    /// TODO Add documentation. [ECR-2820]
    fn new(index_access: T) -> Self {
        let address = IndexAddress::with_root(index_access.root()).append_name(INDEX_ID_POOL_NAME);
        Self {
            view: View::new(index_access, address),
        }
    }

    /// TODO Add documentation. [ECR-2820]
    fn index_id(&self, index_address: &IndexAddress) -> Option<IndexAddress> {
        let path = index_address.to_path();
        self.view.get(&path).map(|id| self.index_id_to_address(id))
    }

    /// TODO Add documentation. [ECR-2820]
    fn root_address(&self) -> IndexAddress {
        IndexAddress::with_root(self.view.index_access.root())
    }

    /// TODO Add documentation. [ECR-2820]
    fn index_id_to_address(&self, id: u64) -> IndexAddress {
        self.root_address().append_bytes(&id)
    }

    /// TODO Add documentation. [ECR-2820]
    fn into_inner(self) -> T {
        self.view.index_access
    }
}

impl IndexIdPool<&Fork> {
    /// TODO Add documentation. [ECR-2820]
    fn create_index_id(&mut self, index_address: &IndexAddress) -> IndexAddress {
        let index_id = {
            let mut len_view = View::new(
                self.view.index_access,
                self.root_address().append_name(INDEX_ID_POOL_LEN),
            );

            let len = len_view.get(&()).unwrap_or_default();
            len_view.put(&(), len + 1);
            len
        };
        self.view.put(&index_address.to_path(), index_id);
        self.index_id_to_address(index_id)
    }
}

/// TODO Add documentation. [ECR-2820]
pub fn get_index_id<T, I>(index_access: T, index_address: I) -> IndexAddress
where
    T: IndexAccess,
    I: Into<IndexAddress>,
{
    let index_address = index_address.into();

    let index_access = {
        let index_id_pool = IndexIdPool::new(index_access);
        if let Some(index_id) = index_id_pool.index_id(&index_address) {
            return index_id;
        } else {
            index_id_pool.into_inner()
        }
    };

    // Unsafe method `index_access.fork()` here is safe because we never use fork outside this block.
    #[allow(unsafe_code)]
    unsafe {
        let root = index_access.root().to_owned();
        if let Some(index_access_mut) = index_access.fork() {
            let mut index_id_pool = IndexIdPool::new(index_access_mut);
            index_id_pool.create_index_id(&index_address)
        } else {
            IndexAddress::with_root(root).append_bytes(&0_u64)
        }
    }
}
