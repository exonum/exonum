#[derive(Debug)]
pub enum Error {
    UnexpectedlyShortPayload { actual_size: u32, minimum_size: u32 },
    IncorrectBoolean { position: u32, value: u8 },
    IncorrectSegmentRefference { position: u32, value: u32 },
    IncorrectSegmentSize { position: u32, value: u32 },
    Utf8 {
        position: u32,
        error: ::std::str::Utf8Error,
    },
}
