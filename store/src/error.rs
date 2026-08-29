#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DecodeError {
    UnexpectedEof,
    VarintTooLong,
    BadMagic,
    UnsupportedVersion(u8, u8),
    UnsupportedKind(u8),
    BadBool(u8),
    BadUtf8,
    BadTag(u8),
    BadNodeRef(u32),
    MissingChild,
    EmptyNodeTable,
    BadRootIndex,
    HashMismatch,
    TrailingBytes,
    EmptyDocument,
    DuplicateDefinition,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DecodeError {}
