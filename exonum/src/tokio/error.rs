use std::error::Error as StdError;
use std::io;

// Common error helpers

pub fn other_error<S: AsRef<str>>(s: S) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s.as_ref())
}

pub fn forget_result<T>(_: T) {}

pub fn result_ok<T, E: StdError>(_: T) -> Result<(), E> {
    Ok(())
}

pub fn log_error<E: StdError>(err: E) {
    error!("An error occured: {}", err)
}

pub fn into_other<E: StdError>(err: E) -> io::Error {
    other_error(&format!("An error occured, {}", err.description()))
}
