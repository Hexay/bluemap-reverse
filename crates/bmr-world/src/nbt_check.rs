//! Structural check of untrusted NBT before fastnbt sees it. fastnbt 2.6 recurses once per nesting level with no
//! limit (deep input overflows the stack) and panics on a non-empty list of `End` tags.

use anyhow::{Result, bail, ensure};

/// Minecraft's own limit (`NbtAccounter`).
const MAX_DEPTH: usize = 512;
const END: u8 = 0;
const LIST: u8 = 9;
const COMPOUND: u8 = 10;

enum Frame {
    Compound,
    List { tag: u8, left: u32 },
}

pub fn check(nbt: &[u8]) -> Result<()> {
    let mut r = Cursor { buf: nbt, pos: 0 };
    ensure!(r.u8()? == COMPOUND, "NBT root is not a compound");
    r.skip_string()?;
    let mut stack = vec![Frame::Compound];
    while let Some(top) = stack.last_mut() {
        let tag = match top {
            Frame::Compound => match r.u8()? {
                END => {
                    stack.pop();
                    continue;
                }
                tag => {
                    r.skip_string()?;
                    tag
                }
            },
            Frame::List { left: 0, .. } => {
                stack.pop();
                continue;
            }
            Frame::List { tag, left } => {
                *left -= 1;
                *tag
            }
        };
        if let Some(frame) = r.value(tag)? {
            ensure!(stack.len() < MAX_DEPTH, "NBT nested deeper than {MAX_DEPTH}");
            stack.push(frame);
        }
    }
    Ok(())
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let Some(s) = self.pos.checked_add(n).and_then(|end| self.buf.get(self.pos..end)) else {
            bail!("NBT truncated at {}", self.pos);
        };
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn len(&mut self) -> Result<usize> {
        let n = i32::from_be_bytes(self.take(4)?.try_into()?);
        ensure!(n >= 0, "negative NBT length {n}");
        Ok(n as usize)
    }

    fn skip_string(&mut self) -> Result<()> {
        let n = u16::from_be_bytes(self.take(2)?.try_into()?);
        self.take(n as usize).map(drop)
    }

    fn skip_array(&mut self, width: usize) -> Result<()> {
        let n = self.len()?;
        self.take(n * width).map(drop)
    }

    /// Skips a scalar or array payload; a list or compound is returned for the caller to descend into.
    fn value(&mut self, tag: u8) -> Result<Option<Frame>> {
        match tag {
            1 => self.take(1).map(drop)?,
            2 => self.take(2).map(drop)?,
            3 | 5 => self.take(4).map(drop)?,
            4 | 6 => self.take(8).map(drop)?,
            7 => self.skip_array(1)?,
            8 => self.skip_string()?,
            11 => self.skip_array(4)?,
            12 => self.skip_array(8)?,
            LIST => {
                let tag = self.u8()?;
                let left = self.len()?;
                ensure!(tag <= 12, "unknown NBT list tag {tag}");
                ensure!(tag != END || left == 0, "NBT list of {left} End tags");
                return Ok(Some(Frame::List { tag, left: left as u32 }));
            }
            COMPOUND => return Ok(Some(Frame::Compound)),
            t => bail!("unknown NBT tag {t}"),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Root compound (empty name) holding `body` as its only tag, named "a".
    fn root(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut v = vec![COMPOUND, 0, 0, tag, 0, 1, b'a'];
        v.extend(body);
        v.push(END);
        v
    }

    #[test]
    fn well_formed_nbt_passes() {
        // list of two compounds: {"": 1i32} and {}
        let body = [COMPOUND, 0, 0, 0, 2, 3, 0, 0, 0, 0, 0, 1, END, END];
        check(&root(LIST, &body)).unwrap();
        check(&root(LIST, &[END, 0, 0, 0, 0])).unwrap();
        check(&root(12, &[0, 0, 0, 1, 1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
    }

    #[test]
    fn non_empty_list_of_end_is_an_error() {
        let e = check(&root(LIST, &[END, 0, 0, 0, 3])).unwrap_err();
        assert!(e.to_string().contains("3 End tags"), "{e}");
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_stack_overflow() {
        let mut body = Vec::new();
        for _ in 0..100_000 {
            body.extend([LIST, 0, 0, 0, 1]);
        }
        let e = check(&root(LIST, &body)).unwrap_err();
        assert!(e.to_string().contains("deeper than 512"), "{e}");
    }

    #[test]
    fn truncated_and_negative_lengths_are_errors() {
        assert!(check(&root(12, &[0, 0, 1, 0])).is_err());
        assert!(check(&root(7, &[0xff, 0xff, 0xff, 0xff])).is_err());
        assert!(check(&[COMPOUND, 0]).is_err());
        assert!(check(&root(13, &[])).is_err());
    }
}
