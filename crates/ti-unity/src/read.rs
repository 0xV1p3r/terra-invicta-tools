//! Bounds-checked cursor over a byte slice.

use crate::{Result, format_err};

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
    pub big_endian: bool,
}

macro_rules! read_num {
    ($name:ident, $t:ty) => {
        pub fn $name(&mut self) -> Result<$t> {
            let b = self.take(size_of::<$t>())?.try_into().unwrap();
            Ok(if self.big_endian {
                <$t>::from_be_bytes(b)
            } else {
                <$t>::from_le_bytes(b)
            })
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], big_endian: bool) -> Self {
        Self { data, pos: 0, big_endian }
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let end = end.ok_or_else(|| format_err(format!("read of {n} bytes at {} past the end ({})", self.pos, self.data.len())))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// A [`Reader::len`] count of entries that take at least `min_size` bytes each, checked
    /// against the bytes left so it can size an allocation.
    pub fn count(&mut self, min_size: usize) -> Result<usize> {
        let n = self.len()?;
        if n.saturating_mul(min_size) > self.remaining() {
            return Err(format_err(format!("count {n} past the end of the data")));
        }
        Ok(n)
    }

    pub fn align(&mut self, n: usize) {
        self.pos = self.pos.div_ceil(n) * n;
    }

    read_num!(u8, u8);
    read_num!(i8, i8);
    read_num!(u16, u16);
    read_num!(i16, i16);
    read_num!(u32, u32);
    read_num!(i32, i32);
    read_num!(u64, u64);
    read_num!(i64, i64);
    read_num!(f32, f32);
    read_num!(f64, f64);

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    /// Non-negative `i32` length.
    pub fn len(&mut self) -> Result<usize> {
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| format_err(format!("negative length {n}")))
    }

    pub fn cstring(&mut self) -> Result<String> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let n = rest.iter().position(|&b| b == 0).ok_or_else(|| format_err("unterminated string"))?;
        self.pos += n + 1;
        Ok(String::from_utf8_lossy(&rest[..n]).into_owned())
    }

    /// `i32` length, bytes, aligned to 4.
    pub fn aligned_string(&mut self) -> Result<String> {
        let n = self.len()?;
        let s = String::from_utf8_lossy(self.take(n)?).into_owned();
        self.align(4);
        Ok(s)
    }
}
