#[macro_export]
macro_rules! counter {
    () => (0usize);
    ($head:ident $($tail:ident)*) => (1usize + counter!($($tail)*))
}