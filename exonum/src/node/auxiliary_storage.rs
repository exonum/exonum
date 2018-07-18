use std::sync::Arc;

use storage::Database;

pub struct AuxiliaryStorage(Arc<Database>);

pub struct ServiceSchema<T> {
    view: T,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {

    }
}
