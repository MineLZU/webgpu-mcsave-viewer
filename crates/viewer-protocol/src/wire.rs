//! Little-endian primitive reads and writes, and the error every protocol operation ends with.

use std::fmt;

/// Why a message could not be decoded, encoded, or accepted by a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// The message ended `needed` bytes short of a field that starts at `offset`.
    Truncated { offset: usize, needed: usize },
    /// The message decoded completely with `count` bytes left over.
    TrailingBytes { count: usize },
    /// The first byte names no message this protocol version defines.
    UnknownMessageType(u8),
    /// A field breaks the message layout; the text says which field and why.
    Invalid(String),
    /// The message is well formed but refers to something the session does not have.
    OutOfBounds(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { offset, needed } => {
                write!(
                    f,
                    "message truncated: {needed} more bytes needed at offset {offset}"
                )
            }
            Self::TrailingBytes { count } => write!(f, "{count} trailing bytes after the message"),
            Self::UnknownMessageType(tag) => write!(f, "unknown message type 0x{tag:02x}"),
            Self::Invalid(reason) => write!(f, "invalid message: {reason}"),
            Self::OutOfBounds(reason) => write!(f, "out of session bounds: {reason}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub(crate) fn invalid(reason: impl Into<String>) -> ProtocolError {
    ProtocolError::Invalid(reason.into())
}

/// A cursor over one message. Every read is bounds-checked; [`Reader::finish`] rejects leftovers.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        let remaining = self.bytes.len() - self.offset;
        if count > remaining {
            return Err(ProtocolError::Truncated {
                offset: self.offset,
                needed: count - remaining,
            });
        }
        let slice = &self.bytes[self.offset..self.offset + count];
        self.offset += count;
        Ok(slice)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        Ok(self.take(N)?.try_into().expect("take returned N bytes"))
    }

    pub(crate) fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, ProtocolError> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    /// A `u16` byte length followed by that many bytes of UTF-8.
    pub(crate) fn string(&mut self) -> Result<String, ProtocolError> {
        let length = self.u16()? as usize;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid("string is not UTF-8"))
    }

    pub(crate) fn finish(self) -> Result<(), ProtocolError> {
        match self.bytes.len() - self.offset {
            0 => Ok(()),
            count => Err(ProtocolError::TrailingBytes { count }),
        }
    }
}

/// The encoding counterpart of [`Reader`], appending to one message buffer.
#[derive(Default)]
pub(crate) struct Writer {
    pub(crate) bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn string(&mut self, value: &str) -> Result<(), ProtocolError> {
        let length =
            u16::try_from(value.len()).map_err(|_| invalid("string longer than 65535 bytes"))?;
        self.u16(length);
        self.bytes(value.as_bytes());
        Ok(())
    }
}
