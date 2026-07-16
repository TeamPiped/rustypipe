//! Minimal UMP (YouTube Media Protocol) parser.
//!
//! Each UMP message is a sequence of parts. Every part has a
//! `UMPPartId` followed by a length-prefixed payload:
//!
//! ```text
//! varint type_id
//! varint payload_size
//! [payload_size bytes] payload
//! ```

use crate::proto::video_streaming::UmpPartId;
use crate::Bytes;
use bytes::Buf;

/// A single segment (part) of a UMP stream.
///
/// Each part has an identifying type and may have associated data.
#[derive(Debug, PartialEq, Eq)]
pub struct Part {
    /// Type of the part.
    ///
    /// Set to [`UmpPartId::Unknown`] if the type could not be identified.
    pub ty: UmpPartId,
    /// Associated data of the part.
    pub data: Bytes,
}

/// A parser to read UMP data.
pub struct Parser {
    buf: Bytes,
}

impl Parser {
    /// Create a new parser for a UMP response.
    pub fn new(buf: Bytes) -> Self {
        Self { buf }
    }

    fn read_byte(&mut self) -> Option<u8> {
        let byte = *self.buf.first()?;
        self.buf.advance(1);
        Some(byte)
    }

    /// Read a variable sized integer from the buffer.
    ///
    /// Follows the same encoding as
    /// <https://github.com/gsuberland/UMP_Format/blob/main/UMP_Format.md#variable-sized-integer>.
    pub fn read_varint(&mut self) -> Option<u32> {
        let prefix = self.read_byte()?;

        // [0..4] leading ones corresponds to 1..5 byte payload
        let varint_size = (prefix.leading_ones() as usize).min(4) + 1;

        let mut shift = 0;
        let mut result = 0u32;

        if varint_size != 5 {
            shift = 8 - varint_size;
            let mask = (1u32 << shift) - 1;
            result |= (prefix as u32) & mask;
        }

        for _ in 1..varint_size {
            result |= (self.read_byte()? as u32) << shift;
            shift += 8;
        }
        Some(result)
    }

    /// Returns the remaining unparsed data of the buffer.
    pub fn data(self) -> Bytes {
        self.buf
    }

    /// Read a single [`Part`].
    pub fn read_part(&mut self) -> Option<Part> {
        let ty = self.read_varint()?;
        let ump_type = UmpPartId::try_from(ty as i32).unwrap_or(UmpPartId::Unknown);

        let size = self.read_varint()?;
        // TODO: data may only be partially contained within a single response
        // segment.
        let data = self.buf.split_to(size as usize);

        Some(Part { ty: ump_type, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_varint() {
        let cases: Vec<(Vec<u8>, u32)> = vec![
            (vec![0x01], 1),
            (vec![0x4f], 79),
            (vec![0x96, 0x00], 22),
            (vec![0x80, 0x01], 64),
            (vec![0x8a, 0x7f], 8138),
            (vec![0xbf, 0x7f], 8191),
            (vec![0xc0, 0x80, 0x01], 12288),
            (vec![0xdf, 0x7f, 0xff], 2093055),
            (vec![0xe0, 0x80, 0x80, 0x01], 1574912),
            (vec![0xef, 0x7f, 0xff, 0xff], 268_433_407),
            (vec![0xf0, 0x80, 0x80, 0x80, 0x01], 25198720),
            (vec![0xff, 0x7f, 0xff, 0xff, 0xff], 4_294_967_167),
        ];
        for (data, expected) in cases {
            let mut parser = Parser::new(Bytes::from(data));
            assert_eq!(Some(expected), parser.read_varint());
        }
    }
}
