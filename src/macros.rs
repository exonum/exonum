/// Creates a `Vec<Box<Transaction>>` from the given transactions.
///
/// Each transaction must have an `Into<Box<Transaction>>` implementation.
#[macro_export]
macro_rules! txvec {
    ($($x:expr),*) => (
        vec![$($x.into()),*]
    );
    ($($x:expr,)*) => (
        vec![$($x.into()),*]
    )
}
