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
