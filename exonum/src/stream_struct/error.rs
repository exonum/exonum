#[derive(Debug)]
pub enum Error {
    UnexpectedlyShortPayload { actual_size: u32, minimum_size: u32 },
    IncorrectBoolean { position: u32, value: u8 },
    IncorrectSegmentReference { position: u32, value: u32 },
    IncorrectSegmentSize { position: u32, value: u32 },
    UnexpectedlyShortRawMessage { position: u32, size: u32 },
    IncorrectSizeOfRawMessage { position: u32, actual_size: u32, declared_size: u32 },
    IncorrectMessageType { position: u32, actual_message_type: u16, declared_message_type: u16 },
    Utf8 {
        position: u32,
        error: ::std::str::Utf8Error,
    },
}
