pub mod cst;
pub mod debug;
pub mod display_error;
pub mod parser;

use crate::{cst::Tree, parser::ParseError};

/// Lex `src`, run the parser, and return `(root_tree, errors)`.
pub fn parse_papyrus(src: &str) -> (Tree, Vec<ParseError>) {
    let tokens = papyrus_token::TokenStream::from(src).into_tokens();
    let mut p = parser::Parser::new(tokens);
    parser::file(&mut p);
    p.build_tree()
}
