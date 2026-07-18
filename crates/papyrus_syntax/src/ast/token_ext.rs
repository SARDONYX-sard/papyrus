//! There are many AstNodes, but only a few tokens, so we hand-write them here.

use std::ops::Range;
use std::{borrow::Cow, num::ParseIntError};

use rustc_literal_escaper::{EscapeError, unescape_str};
use stdx::always;

use crate::{
    TextRange, TextSize,
    ast::{self, AstToken},
};

impl ast::Comment {
    pub fn kind(&self) -> CommentKind {
        CommentKind::from_text(self.text())
    }

    pub fn is_doc(&self) -> bool {
        self.kind().doc
    }

    pub fn is_line(&self) -> bool {
        self.kind().shape == CommentShape::Line
    }

    pub fn is_block(&self) -> bool {
        self.kind().shape == CommentShape::Block
    }

    pub fn prefix(&self) -> &'static str {
        self.kind().prefix()
    }

    pub fn doc_comment(&self) -> Option<(&str, TextSize)> {
        let kind = self.kind();

        if !kind.doc {
            return None;
        }

        let prefix = kind.prefix();
        let text = &self.text()[prefix.len()..];

        let text = text.trim_end();
        let text = text.strip_suffix('}').unwrap_or(text);

        Some((text, TextSize::of(prefix)))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct CommentKind {
    pub shape: CommentShape,
    pub doc: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CommentShape {
    Line,
    Block,
}

impl CommentKind {
    const BY_PREFIX: [(&'static str, CommentKind); 3] = [
        ("{", CommentKind { shape: CommentShape::Block, doc: true }),
        (";\\", CommentKind { shape: CommentShape::Block, doc: false }),
        (";", CommentKind { shape: CommentShape::Line, doc: false }),
    ];

    pub(crate) fn from_text(text: &str) -> CommentKind {
        Self::BY_PREFIX
            .iter()
            .find_map(|(prefix, kind)| text.starts_with(prefix).then_some(*kind))
            .unwrap()
    }

    pub fn prefix(&self) -> &'static str {
        Self::BY_PREFIX
            .iter()
            .find_map(|(prefix, kind)| (*kind == *self).then_some(*prefix))
            .unwrap()
    }
}

impl ast::Whitespace {
    pub fn spans_multiple_lines(&self) -> bool {
        let text = self.text();
        text.find('\n').is_some_and(|idx| text[idx + 1..].contains('\n'))
    }
}

#[derive(Debug)]
pub struct QuoteOffsets {
    pub quotes: (TextRange, TextRange),
    pub contents: TextRange,
}

impl QuoteOffsets {
    fn new(literal: &str) -> Option<QuoteOffsets> {
        let left_quote = literal.find('"')?;
        let right_quote = literal.rfind('"')?;
        if left_quote == right_quote {
            // `literal` only contains one quote
            return None;
        }

        let start = TextSize::from(0);
        let left_quote = TextSize::try_from(left_quote).unwrap() + TextSize::of('"');
        let right_quote = TextSize::try_from(right_quote).unwrap();
        let end = TextSize::of(literal);

        let res = QuoteOffsets {
            quotes: (TextRange::new(start, left_quote), TextRange::new(right_quote, end)),
            contents: TextRange::new(left_quote, right_quote),
        };
        Some(res)
    }
}

#[expect(unused)]
pub trait IsString: AstToken {
    fn raw_prefix(&self) -> &'static str;
    fn unescape(&self, s: &str, callback: impl FnMut(Range<usize>, Result<char, EscapeError>));
    fn is_raw(&self) -> bool {
        self.text().starts_with(self.raw_prefix())
    }
    fn quote_offsets(&self) -> Option<QuoteOffsets> {
        let text = self.text();
        let offsets = QuoteOffsets::new(text)?;
        let o = self.syntax().text_range().start();
        let offsets = QuoteOffsets {
            quotes: (offsets.quotes.0 + o, offsets.quotes.1 + o),
            contents: offsets.contents + o,
        };
        Some(offsets)
    }
    fn text_range_between_quotes(&self) -> Option<TextRange> {
        self.quote_offsets().map(|it| it.contents)
    }
    fn text_without_quotes(&self) -> &str {
        let text = self.text();
        let Some(offsets) = self.text_range_between_quotes() else { return text };
        &text[offsets - self.syntax().text_range().start()]
    }
    fn open_quote_text_range(&self) -> Option<TextRange> {
        self.quote_offsets().map(|it| it.quotes.0)
    }
    fn close_quote_text_range(&self) -> Option<TextRange> {
        self.quote_offsets().map(|it| it.quotes.1)
    }
    fn escaped_char_ranges(&self, cb: &mut dyn FnMut(TextRange, Result<char, EscapeError>)) {
        let Some(text_range_no_quotes) = self.text_range_between_quotes() else { return };

        let start = self.syntax().text_range().start();
        let text = &self.text()[text_range_no_quotes - start];
        let offset = text_range_no_quotes.start() - start;

        self.unescape(text, &mut |range: Range<usize>, unescaped_char| {
            if let Some((s, e)) = range.start.try_into().ok().zip(range.end.try_into().ok()) {
                cb(TextRange::new(s, e) + offset, unescaped_char);
            }
        });
    }
    fn map_range_up(&self, range: TextRange) -> Option<TextRange> {
        let contents_range = self.text_range_between_quotes()?;
        if always!(TextRange::up_to(contents_range.len()).contains_range(range)) {
            Some(range + contents_range.start())
        } else {
            None
        }
    }
    fn map_offset_down(&self, offset: TextSize) -> Option<TextSize> {
        let contents_range = self.text_range_between_quotes()?;
        offset.checked_sub(contents_range.start())
    }
}

impl IsString for ast::String {
    fn raw_prefix(&self) -> &'static str {
        "r"
    }
    fn unescape(&self, s: &str, cb: impl FnMut(Range<usize>, Result<char, EscapeError>)) {
        unescape_str(s, cb)
    }
}

impl ast::String {
    pub fn value(&self) -> Result<Cow<'_, str>, EscapeError> {
        let text = self.text();
        let text_range = self.text_range_between_quotes().ok_or(EscapeError::LoneSlash)?;
        let text = &text[text_range - self.syntax().text_range().start()];
        if self.is_raw() {
            return Ok(Cow::Borrowed(text));
        }

        let mut buf = String::new();
        let mut prev_end = 0;
        let mut has_error = None;
        unescape_str(text, |char_range, unescaped_char| {
            match (unescaped_char, buf.capacity() == 0) {
                (Ok(c), false) => buf.push(c),
                (Ok(_), true) if char_range.len() == 1 && char_range.start == prev_end => {
                    prev_end = char_range.end
                }
                (Ok(c), true) => {
                    buf.reserve_exact(text.len());
                    buf.push_str(&text[..prev_end]);
                    buf.push(c);
                }
                (Err(e), _) => has_error = Some(e),
            }
        });

        match (has_error, buf.capacity() == 0) {
            (Some(e), _) => Err(e),
            (None, true) => Ok(Cow::Borrowed(text)),
            (None, false) => Ok(Cow::Owned(buf)),
        }
    }
}

impl ast::IntNumber {
    pub fn radix(&self) -> Radix {
        match self.text().get(..2).unwrap_or_default() {
            "0b" => Radix::Binary,
            "0o" => Radix::Octal,
            "0x" => Radix::Hexadecimal,
            _ => Radix::Decimal,
        }
    }

    pub fn split_into_parts(&self) -> (&str, &str, &str) {
        let radix = self.radix();
        let (prefix, mut text) = self.text().split_at(radix.prefix_len());

        let is_suffix_start: fn(&(usize, char)) -> bool = match radix {
            Radix::Hexadecimal => |(_, c)| matches!(c, 'g'..='z' | 'G'..='Z'),
            _ => |(_, c)| c.is_ascii_alphabetic(),
        };

        let mut suffix = "";
        if let Some((suffix_start, _)) = text.char_indices().find(is_suffix_start) {
            let (text2, suffix2) = text.split_at(suffix_start);
            text = text2;
            suffix = suffix2;
        };

        (prefix, text, suffix)
    }

    pub fn value(&self) -> Result<u128, ParseIntError> {
        let (_, text, _) = self.split_into_parts();
        u128::from_str_radix(&text.replace('_', ""), self.radix() as u32)
    }

    pub fn suffix(&self) -> Option<&str> {
        let (_, _, suffix) = self.split_into_parts();
        if suffix.is_empty() { None } else { Some(suffix) }
    }

    pub fn value_string(&self) -> String {
        let (_, text, _) = self.split_into_parts();
        text.replace('_', "")
    }
}

impl ast::FloatNumber {
    pub fn split_into_parts(&self) -> (&str, &str) {
        let text = self.text();
        let mut float_text = self.text();
        let mut suffix = "";
        let mut indices = text.char_indices();
        if let Some((mut suffix_start, c)) = indices.by_ref().find(|(_, c)| c.is_ascii_alphabetic())
        {
            if c == 'e' || c == 'E' {
                if let Some(suffix_start_tuple) = indices.find(|(_, c)| c.is_ascii_alphabetic()) {
                    suffix_start = suffix_start_tuple.0;

                    float_text = &text[..suffix_start];
                    suffix = &text[suffix_start..];
                }
            } else {
                float_text = &text[..suffix_start];
                suffix = &text[suffix_start..];
            }
        }

        (float_text, suffix)
    }

    pub fn suffix(&self) -> Option<&str> {
        let (_, suffix) = self.split_into_parts();
        if suffix.is_empty() { None } else { Some(suffix) }
    }

    pub fn value_string(&self) -> String {
        let (text, _) = self.split_into_parts();
        text.replace('_', "")
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Radix {
    Binary = 2,
    Octal = 8,
    Decimal = 10,
    Hexadecimal = 16,
}

impl Radix {
    pub const ALL: &'static [Radix] =
        &[Radix::Binary, Radix::Octal, Radix::Decimal, Radix::Hexadecimal];

    const fn prefix_len(self) -> usize {
        match self {
            Self::Decimal => 0,
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{AstNode as _, SourceFile};

    use super::*;

    fn comment_from_text(text: &str) -> ast::Comment {
        let parse = SourceFile::parse(text, &[]);

        parse
            .tree()
            .syntax()
            .children_with_tokens()
            .find_map(|it| match it {
                rowan::NodeOrToken::Token(token) => ast::Comment::cast(token),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn doc_comment_text() {
        let src = r#"
ScriptName Test
{ This is documentation. }
"#;

        let comment = comment_from_text(src);

        assert!(comment.is_doc());

        let (text, offset) = comment.doc_comment().unwrap();

        assert_eq!(text.trim(), "This is documentation.");
        assert_eq!(offset, TextSize::of("{"));
    }
}
