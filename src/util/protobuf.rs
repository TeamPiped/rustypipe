/// [`ProtoBuilder`] is used to construct protobuf messages using a builder pattern
#[derive(Debug, Default)]
pub struct ProtoBuilder {
    pub bytes: Vec<u8>,
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
        let fbits: u64 = (field as u64) << 3;
        let wbits = wire as u64 & 0x07;
        let val: u64 = fbits | wbits;
        self._varint(val);
    }

    /// Write a varint field
    pub fn varint(&mut self, field: u32, val: u64) {
        self._field(field, 0);
        self._varint(val);
    }

    /// Write an embedded message
    ///
    /// Requires passing another [`ProtoBuilder`] with the embedded message.
    pub fn embedded(&mut self, field: u32, mut pb: Self) {
        self._field(field, 2);
        self._varint(pb.bytes.len() as u64);
        self.bytes.append(&mut pb.bytes);
    }
}

fn parse_varint<P: Iterator<Item = u8>>(pb: &mut P) -> Option<u64> {
    let mut result = 0;
    let mut num_read = 0;

    for b in pb.by_ref() {
        let value = b & 0x7f;
        result |= (value as u64) << (7 * num_read);
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
                let len = some_or_bail!(parse_varint(&mut pb), None);
                if this_field == field {
                    let mut buf = Vec::new();
                    for _ in 0..len {
                        buf.push(some_or_bail!(pb.next(), None));
                    }
                    return String::from_utf8(buf).ok();
                } else {
                    len
                }
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
    use super::*;

    // #[test]
    // fn t_parse_varint() {

    // }

    #[test]
    fn t_parse_proto() {
        let p = "GhhVQzl2cnZOU0wzeGNXR1NrVjg2UkVCU2c%3D";
        let p_bytes = base64::decode(urlencoding::decode(p).unwrap().as_bytes()).unwrap();

        let res = string_from_pb(p_bytes, 3).unwrap();
        assert_eq!(res, "UC9vrvNSL3xcWGSkV86REBSg");
    }
}
