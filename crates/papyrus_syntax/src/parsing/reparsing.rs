//! Implementation of incremental re-parsing.
//!
//! We use two simple strategies for this:
//!   - if the edit modifies only a single token (like changing an identifier's
//!     letter), we replace only this token.
//!   - otherwise, we search for the nearest `{}` block which contains the edit
//!     and try to parse only this block.

use std::ops::Range;

use papyrus_parser::{self as parser, Reparser};

use crate::{
    SyntaxError,
    SyntaxKind::*,
    TextRange, TextSize,
    parsing::build_tree,
    syntax_node::{GreenNode, GreenToken, NodeOrToken, SyntaxElement, SyntaxNode},
};

pub(crate) fn incremental_reparse(
    node: &SyntaxNode,
    delete: TextRange,
    insert: &str,
    errors: impl IntoIterator<Item = SyntaxError>,
) -> Option<(GreenNode, Vec<SyntaxError>, TextRange)> {
    if let Some((green, new_errors, old_range)) = reparse_token(node, delete, insert) {
        return Some((
            green,
            merge_errors(errors, new_errors, old_range, delete, insert),
            old_range,
        ));
    }

    if let Some((green, new_errors, old_range)) = reparse_block(node, delete, insert) {
        return Some((
            green,
            merge_errors(errors, new_errors, old_range, delete, insert),
            old_range,
        ));
    }
    None
}

fn reparse_token(
    root: &SyntaxNode,
    delete: TextRange,
    insert: &str,
) -> Option<(GreenNode, Vec<SyntaxError>, TextRange)> {
    let prev_token = root.covering_element(delete).as_token()?.clone();
    let prev_token_kind = prev_token.kind();
    match prev_token_kind {
        WHITESPACE | COMMENT | IDENT | STRING => {
            if prev_token_kind == WHITESPACE || prev_token_kind == COMMENT {
                // removing a new line may extends previous token
                let deleted_range = delete - prev_token.text_range().start();
                if prev_token.text()[deleted_range].contains('\n') {
                    return None;
                }
            }

            let mut new_text = get_text_after_edit(prev_token.clone().into(), delete, insert);
            let (new_token_kind, new_err) = parser::LexedStr::single_token(&new_text)?;

            if new_token_kind != prev_token_kind
                || (new_token_kind == IDENT && is_contextual_kw(&new_text))
            {
                return None;
            }

            // Check that edited token is not a part of the bigger token.
            // E.g. if for source code `bruh"str"` the user removed `ruh`, then
            // `b` no longer remains an identifier, but becomes a part of byte string literal
            if let Some(next_char) = root.text().char_at(prev_token.text_range().end()) {
                new_text.push(next_char);
                let token_with_next_char = parser::LexedStr::single_token(&new_text);
                if let Some((_kind, _error)) = token_with_next_char {
                    return None;
                }
                new_text.pop();
            }

            let new_token = GreenToken::new(rowan::SyntaxKind(prev_token_kind.into()), &new_text);
            let range = TextRange::up_to(TextSize::of(&new_text));
            Some((
                prev_token.replace_with(new_token),
                new_err.into_iter().map(|msg| SyntaxError::new(msg, range)).collect(),
                prev_token.text_range(),
            ))
        }
        _ => None,
    }
}

fn reparse_block(
    root: &SyntaxNode,
    delete: TextRange,
    insert: &str,
) -> Option<(GreenNode, Vec<SyntaxError>, TextRange)> {
    let (node, reparser) = find_reparsable_node(root, delete)?;
    let text = get_text_after_edit(node.clone().into(), delete, insert);

    let lexed = parser::LexedStr::new(text.as_str());
    let parser_input = lexed.to_input();
    // if !is_balanced(&lexed) {
    //     return None;
    // }

    let tree_traversal = reparser.parse(&parser_input);

    let (green, new_parser_errors, _eof) = build_tree(lexed, tree_traversal);

    Some((node.replace_with(green), new_parser_errors, node.text_range()))
}

fn get_text_after_edit(element: SyntaxElement, mut delete: TextRange, insert: &str) -> String {
    delete -= element.text_range().start();

    let mut text = match element {
        NodeOrToken::Token(token) => token.text().to_owned(),
        NodeOrToken::Node(node) => node.text().to_string(),
    };
    text.replace_range(Range::<usize>::from(delete), insert);
    text
}

fn is_contextual_kw(text: &str) -> bool {
    matches!(text, "auto" | "default" | "union")
}

fn find_reparsable_node(node: &SyntaxNode, range: TextRange) -> Option<(SyntaxNode, Reparser)> {
    let node = node.covering_element(range);

    node.ancestors().find_map(|node| {
        let first_child = node.first_child_or_token().map(|it| it.kind());
        let parent = node.parent().map(|it| it.kind());
        Reparser::for_node(node.kind(), first_child, parent).map(|r| (node, r))
    })
}

// fn is_balanced(lexed: &parser::LexedStr<'_>) -> bool {
//     if lexed.is_empty() || lexed.kind(0) != T!['{'] || lexed.kind(lexed.len() - 1) != T!['}'] {
//         return false;
//     }
//     let mut balance = 0usize;
//     for i in 1..lexed.len() - 1 {
//         match lexed.kind(i) {
//             T!['{'] => balance += 1,
//             T!['}'] => {
//                 balance = match balance.checked_sub(1) {
//                     Some(b) => b,
//                     None => return false,
//                 }
//             }
//             _ => (),
//         }
//     }
//     balance == 0
// }

fn merge_errors(
    old_errors: impl IntoIterator<Item = SyntaxError>,
    new_errors: Vec<SyntaxError>,
    range_before_reparse: TextRange,
    delete: TextRange,
    insert: &str,
) -> Vec<SyntaxError> {
    let mut res = Vec::new();

    for old_err in old_errors {
        let old_err_range = old_err.range();
        if old_err_range.end() <= range_before_reparse.start() {
            res.push(old_err);
        } else if old_err_range.start() >= range_before_reparse.end() {
            let inserted_len = TextSize::of(insert);
            res.push(old_err.with_range((old_err_range + inserted_len) - delete.len()));
            // Note: extra parens are intentional to prevent uint underflow, HWAB (here was a bug)
        }
    }
    res.extend(new_errors.into_iter().map(|new_err| {
        // fighting borrow checker with a variable ;)
        let offsetted_range = new_err.range() + range_before_reparse.start();
        new_err.with_range(offsetted_range)
    }));
    res
}
