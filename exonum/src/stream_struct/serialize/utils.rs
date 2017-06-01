#[macro_export]
/// Calculate num of idents in macro call.
/// Used by `message!` and `storage_value!`
macro_rules! indents_count {
    () => (0usize);
    ($head:ident $($tail:ident)*) => (1usize + indents_count!($($tail)*))
}
