//! Source text span.

use core::ops::Range;

/// Byte offset in the source file.
pub type TextSize = u32;

/// Half-open byte range `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextSpan {
    pub start: TextSize,
    pub end: TextSize,
}

impl TextSpan {
    #[inline]
    pub const fn new(start: TextSize, end: TextSize) -> Self {
        Self { start, end }
    }

    #[inline]
    pub const fn empty(offset: TextSize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    #[inline]
    pub const fn len(self) -> TextSize {
        self.end - self.start
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    #[inline]
    pub const fn contains(self, offset: TextSize) -> bool {
        offset >= self.start && offset < self.end
    }

    #[inline]
    pub const fn contains_span(self, other: TextSpan) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    #[inline]
    pub const fn merge(self, other: TextSpan) -> TextSpan {
        TextSpan {
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        }
    }

    #[inline]
    pub const fn as_range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

impl From<Range<usize>> for TextSpan {
    #[inline]
    fn from(value: Range<usize>) -> Self {
        Self {
            start: value.start as TextSize,
            end: value.end as TextSize,
        }
    }
}

impl From<TextSpan> for Range<usize> {
    #[inline]
    fn from(span: TextSpan) -> Self {
        span.start as usize..span.end as usize
    }
}
