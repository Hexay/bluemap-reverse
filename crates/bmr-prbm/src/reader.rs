//! Little-endian cursor over a PRBM buffer. Alignment is measured from the start of the buffer.

use anyhow::{Result, bail};

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len());
        let Some(end) = end else {
            bail!("unexpected end of PRBM at {} (+{n}, len {})", self.pos, self.buf.len());
        };
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u24(&mut self) -> Result<u32> {
        let b = self.bytes(3)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], 0]))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into()?))
    }

    pub fn cstr(&mut self) -> Result<&'a str> {
        let rest = &self.buf[self.pos..];
        let Some(len) = rest.iter().position(|&b| b == 0) else { bail!("unterminated string") };
        let s = std::str::from_utf8(&rest[..len])?;
        self.pos += len + 1;
        Ok(s)
    }

    pub fn align4(&mut self) {
        self.pos = self.pos.next_multiple_of(4);
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.buf.len()
    }
}
