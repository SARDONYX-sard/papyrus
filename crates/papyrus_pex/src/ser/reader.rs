use crate::{Error, pex::*};

pub struct Reader<'a> {
    original: &'a [u8],
    remaining: &'a [u8],
    string_table: Vec<&'a str>,

    #[cfg(feature = "trace-layout")]
    pub(crate) annotations: Vec<Annotation>,
}

#[cfg(feature = "trace-layout")]
pub struct Annotation {
    pub offset: usize,
    pub size: usize,
    pub label: String,
}

#[cfg(feature = "trace-layout")]
impl<'a> Reader<'a> {
    #[inline]
    pub fn annotate(&mut self, start: usize, size: usize, label: impl Into<String>) {
        self.annotations.push(Annotation { offset: start, size, label: label.into() });
    }
}

impl<'a> Reader<'a> {
    #[inline]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self {
            original: bytes,
            remaining: bytes,
            string_table: Vec::new(),
            #[cfg(feature = "trace-layout")]
            annotations: Vec::new(),
        }
    }

    #[inline]
    pub fn offset(&self) -> usize {
        self.original.len() - self.remaining.len()
    }

    #[inline]
    pub fn set_string_table(&mut self, string_table: Vec<&'a str>) {
        self.string_table = string_table;
    }

    #[inline]
    pub fn take_string_table(&mut self) -> Vec<&'a str> {
        core::mem::take(&mut self.string_table)
    }

    #[inline]
    pub fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let Some((head, tail)) = self.remaining.split_at_checked(len) else {
            return Err(Error::UnexpectedEof {
                offset: self.offset(),
                expected: len,
                remaining: self.remaining.len(),
            });
        };

        self.remaining = tail;

        Ok(head)
    }

    #[inline]
    pub fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    #[inline]
    pub fn u16(&mut self) -> Result<u16, Error> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    #[inline]
    pub fn u32(&mut self) -> Result<u32, Error> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    #[inline]
    pub fn i32(&mut self) -> Result<i32, Error> {
        let bytes = self.take(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// i64 or time
    #[inline]
    pub fn i64(&mut self) -> Result<i64, Error> {
        let bytes = self.take(8)?;

        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    #[inline]
    pub fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_bits(self.u32()?))
    }

    #[inline]
    pub fn string(&mut self) -> Result<&'a str, Error> {
        let offset = self.offset();

        let len = self.u16()? as usize;
        let bytes = self.take(len)?;

        core::str::from_utf8(bytes).map_err(|source| Error::InvalidUtf8 { offset, source })
    }

    #[inline]
    pub fn string_id(&mut self) -> Result<StringId, Error> {
        Ok(StringId(self.u16()?))
    }
}
