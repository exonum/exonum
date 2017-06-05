#[macro_export]
/// Calculate num of idents in macro call.
/// Used by `message!` and `storage_value!`
macro_rules! idents_count {
    () => (0usize);
    ($head:ident $($tail:ident)*) => (1usize + idents_count!($($tail)*))
}
