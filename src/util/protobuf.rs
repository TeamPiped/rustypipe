/// [`ProtoBuilder`] is used to construct protobuf messages using a builder pattern
#[derive(Debug, Default)]
pub struct ProtoBuilder {
    bytes: Vec<u8>,
}

impl ProtoBuilder {
    /// Instantiate a new [`ProtoBuilder`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Internal: write a raw varint value
    fn _varint(&mut self, val: u64) {
        if val == 0 {
            self.bytes.push(0);
        } else {
            let mut v = val;
            while v != 0 {
                let mut byte = (v & 0x7f) as u8;
                v >>= 7;

                if v != 0 {
                    byte |= 0x80;
                }

                self.bytes.push(byte);
            }
        }
    }

    /// Internal: write a field tag
    ///
    /// Reference: <https://developers.google.com/protocol-buffers/docs/encoding?hl=en#structure>
    fn _field(&mut self, field: u32, wire: u8) {
        let fbits = u64::from(field) << 3;
        let wbits = u64::from(wire) & 0x07;
        let val: u64 = fbits | wbits;
        self._varint(val);
    }

    /// Returns `true` if the builder contains no data
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Write a varint field
    pub fn varint(&mut self, field: u32, val: u64) {
        self._field(field, 0);
        self._varint(val);
    }

    /// Write a string field
    pub fn string(&mut self, field: u32, string: &str) {
        self._field(field, 2);
        self._varint(string.len() as u64);
        self.bytes.extend_from_slice(string.as_bytes());
    }

    /// Write a bytes field
    pub fn bytes(&mut self, field: u32, bytes: &[u8]) {
        self._field(field, 2);
        self._varint(bytes.len() as u64);
        self.bytes.extend_from_slice(bytes);
    }

    /// Write an embedded message
    ///
    /// Requires passing another [`ProtoBuilder`] with the embedded message.
    pub fn embedded(&mut self, field: u32, mut pb: Self) {
        self._field(field, 2);
        self._varint(pb.bytes.len() as u64);
        self.bytes.append(&mut pb.bytes);
    }

    /// Base64 + urlencode the protobuf data
    pub fn to_base64(&self) -> String {
        let b64 = super::b64_encode(&self.bytes);
        urlencoding::encode(&b64).to_string()
    }
}

fn parse_varint<P: Iterator<Item = u8>>(pb: &mut P) -> Option<u64> {
    let mut result = 0;
    let mut num_read = 0;

    for b in pb.by_ref() {
        let value = b & 0x7f;
        result |= u64::from(value) << (7 * num_read);
        num_read += 1;

        if b & 0x80 == 0 {
            break;
        }
    }
    if num_read == 0 {
        None
    } else {
        Some(result)
    }
}

fn parse_field<P: Iterator<Item = u8>>(pb: &mut P) -> Option<(u32, u8)> {
    parse_varint(pb).map(|v| {
        let f = (v >> 3) as u32;
        let w = (v & 0x07) as u8;
        (f, w)
    })
}

pub fn string_from_pb<P: IntoIterator<Item = u8>>(pb: P, field: u32) -> Option<String> {
    let mut pb = pb.into_iter();
    while let Some((this_field, wire)) = parse_field(&mut pb) {
        let to_skip = match wire {
            // varint
            0 => {
                parse_varint(&mut pb);
                0
            }
            // fixed 64bit
            1 => 8,
            // fixed 32bit
            5 => 4,
            // string
            2 => {
                let len = parse_varint(&mut pb)?;
                if this_field == field {
                    let mut buf = Vec::new();
                    for _ in 0..len {
                        buf.push(pb.next()?);
                    }
                    return String::from_utf8(buf).ok();
                }
                len
            }
            _ => return None,
        };
        for _ in 0..to_skip {
            pb.next();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::util;

    use super::*;

    #[test]
    fn t_protobuilder() {
        let mut pb = ProtoBuilder::new();
        pb.varint(1, 128);
        pb.varint(2, 1234567890);
        pb.varint(3, 1234567890123456789);
        pb.string(4, "Hello");
        pb.bytes(5, &[1, 2, 3]);
        assert_eq!(
            pb.to_base64(),
            "CIABENKF2MwEGJWCpu_HnoSRESIFSGVsbG8qAwECAw%3D%3D"
        );
    }

    #[test]
    fn t_parse_proto() {
        let p = "GhhVQzl2cnZOU0wzeGNXR1NrVjg2UkVCU2c%3D";
        let p_bytes = util::b64_decode(urlencoding::decode(p).unwrap().as_bytes()).unwrap();

        let res = string_from_pb(p_bytes, 3).unwrap();
        assert_eq!(res, "UC9vrvNSL3xcWGSkV86REBSg");
    }
}
