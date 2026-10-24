//! The Papyrus grammar.
//!
//! Each function corresponds to a production. Papyrus is line oriented: every
//! declaration header and every statement ends at a [`NEWLINE`] (or at the end of
//! input). This is what removes the ambiguity between trailing flags and the first
//! statement of a function body:
//!
//! ```papyrus
//! Function Foo() Global  ; `Global` is a flag: same line as the signature
//!     Hidden()           ; `Hidden` is a call: it starts a new line
//! EndFunction
//! ```
//!
//! Newline tokens between declarations and statements are children of the
//! enclosing list node (`SOURCE_FILE`, `STATE`, `PROPERTY` or `BLOCK`), never of
//! the declaration or statement itself, so that node ranges stay tight.
//!
//! Error recovery is line based: whenever a production cannot make sense of a
//! line, the rest of the line is wrapped in an `ERROR` node and parsing resumes on
//! the next line.

mod expressions;
mod items;
mod statements;

use crate::{SyntaxKind::*, parser::Parser};

pub(crate) use items::source_file;

/// Consumes the end of a line. Anything left before the newline is reported and
/// wrapped in an `ERROR` node.
fn eol(p: &mut Parser<'_>) {
    if p.at_eol() {
        return;
    }
    p.error_expected("end of line");
    skip_to_eol(p);
}

/// Wraps everything up to the end of the line in an `ERROR` node, without
/// reporting anything.
fn skip_to_eol(p: &mut Parser<'_>) {
    if p.at_eol() {
        return;
    }
    let m = p.open();
    while !p.at_eol() {
        p.advance();
    }
    p.close(m, ERROR);
}

/// `ident`, the name introduced by a declaration.
fn name(p: &mut Parser<'_>) -> bool {
    if p.at(IDENT) {
        let m = p.open();
        p.advance();
        p.close(m, NAME);
        true
    } else {
        p.error_expected("a name");
        false
    }
}

/// `ident | Self | Parent`, a reference to a declaration.
fn name_ref(p: &mut Parser<'_>) {
    debug_assert!(p.at_any(&[IDENT, SELF_KW, PARENT_KW]));
    let m = p.open();
    p.advance();
    p.close(m, NAME_REF);
}

/// The number of tokens of the type starting `n` tokens ahead, if any:
/// `Int` is 1 token long, `Int[]` is 3.
fn type_len_at(p: &Parser<'_>, n: usize) -> Option<usize> {
    let kind = p.nth(n);
    if kind != IDENT && !kind.is_primitive_type() {
        return None;
    }
    if p.nth_at(n + 1, L_BRACK) && p.nth_at(n + 2, R_BRACK) { Some(3) } else { Some(1) }
}

/// `BaseType ('[' ']')?`
fn ty(p: &mut Parser<'_>) {
    let m = p.open();
    if p.at(IDENT) || p.current().is_primitive_type() {
        p.advance();
        if p.at(L_BRACK) && p.nth_at(1, R_BRACK) {
            p.advance();
            p.advance();
        }
    } else {
        p.error_expected("a type");
    }
    p.close(m, TYPE);
}

/// A type without array suffix, used by `New Type[size]`.
fn base_ty(p: &mut Parser<'_>) {
    let m = p.open();
    if p.at(IDENT) || p.current().is_primitive_type() {
        p.advance();
    } else {
        p.error_expected("a type");
    }
    p.close(m, TYPE);
}

/// `'=' Expr`
fn initializer(p: &mut Parser<'_>) {
    debug_assert!(p.at(EQ));
    let m = p.open();
    p.advance();
    expressions::expr(p);
    p.close(m, INITIALIZER);
}

/// Flags seen by [`flags`].
#[derive(Default, Clone, Copy)]
struct SeenFlags {
    native: bool,
    auto: bool,
}

/// `FLAG*`: trailing flags of a declaration header, up to the end of the line.
fn flags(p: &mut Parser<'_>) -> SeenFlags {
    let mut seen = SeenFlags::default();
    loop {
        match p.current() {
            NATIVE_KW => seen.native = true,
            AUTO_KW | AUTOREADONLY_KW => seen.auto = true,
            IDENT | GLOBAL_KW => {}
            _ => return seen,
        }
        let m = p.open();
        p.advance();
        p.close(m, FLAG);
    }
}

/// Tokens that can only appear at the start of a declaration (or end a block),
/// never in a statement. Blocks stop on them, which lets a missing `EndIf` or
/// `EndFunction` be reported without swallowing the rest of the file.
fn at_block_end(p: &Parser<'_>) -> bool {
    match p.current() {
        EOF | ELSE_KW | ELSEIF_KW | ENDIF_KW | ENDWHILE_KW | ENDFUNCTION_KW | ENDEVENT_KW
        | ENDPROPERTY_KW | ENDSTATE_KW | FUNCTION_KW | EVENT_KW | STATE_KW | IMPORT_KW
        | SCRIPTNAME_KW | PROPERTY_KW => true,
        AUTO_KW => p.nth_at(1, STATE_KW),
        _ => match type_len_at(p, 0) {
            Some(n) => matches!(p.nth(n), FUNCTION_KW | PROPERTY_KW),
            None => false,
        },
    }
}
