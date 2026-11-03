// SPDX-License-Identifier: GPL-3.0-only

//! 小端字节游标。

use super::FormatError;

/// 小端字节游标。
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        ByteReader { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn read_u8(&mut self) -> Result<u8, FormatError> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    pub fn read_u16_le(&mut self) -> Result<u16, FormatError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32, FormatError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        if self.remaining() < n {
            return Err(FormatError::UnexpectedEof {
                needed: n,
                remaining: self.remaining(),
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), FormatError> {
        self.read_bytes(n).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_values() {
        let data = [0x01, 0x02, 0x03, 0xFF, 0x06, 0x07, 0x08];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_u8().unwrap(), 0x01);
        assert_eq!(r.read_u16_le().unwrap(), 0x0302);
        assert_eq!(r.read_u32_le().unwrap(), 0x0807_06FF);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn read_bytes_returns_slice_and_advances() {
        let data = b"ABCDEF";
        let mut r = ByteReader::new(data);
        let s = r.read_bytes(3).unwrap();
        assert_eq!(s, b"ABC");
        assert_eq!(r.position(), 3);
        assert_eq!(r.remaining(), 3);
        r.skip(3).unwrap();
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn eof_is_reported() {
        let data = [0x01];
        let mut r = ByteReader::new(&data);
        assert!(matches!(
            r.read_u32_le(),
            Err(FormatError::UnexpectedEof { .. })
        ));
        assert!(r.read_bytes(2).is_err());
    }
}
