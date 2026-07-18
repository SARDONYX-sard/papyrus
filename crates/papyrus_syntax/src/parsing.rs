//! Lexing, bridging to parser (which does the actual parsing) and
//! incremental reparsing.

mod reparsing;

use rowan::TextRange;

use crate::{SyntaxError, SyntaxTreeBuilder, syntax_node::GreenNode};

pub(crate) use crate::parsing::reparsing::incremental_reparse;

pub(crate) fn parse_text(text: &str, custom_flags: &[String]) -> (GreenNode, Vec<SyntaxError>) {
    let _p = tracing::info_span!("parse_text").entered();
    let lexed = papyrus_parser::LexedStr::new(text);
    let parser_input = lexed.to_input(custom_flags);
    let parser_output = papyrus_parser::TopEntryPoint::SourceFile.parse(&parser_input);

    let (node, errors, _eof) = build_tree(lexed, parser_output);
    (node, errors)
}

pub(crate) fn parse_text_at(
    text: &str,
    entry: papyrus_parser::TopEntryPoint,
    custom_flags: &[String],
) -> (GreenNode, Vec<SyntaxError>) {
    let _p = tracing::info_span!("parse_text_at").entered();
    let lexed = papyrus_parser::LexedStr::new(text);
    let parser_input = lexed.to_input(custom_flags);
    let parser_output = entry.parse(&parser_input);
    let (node, errors, _eof) = build_tree(lexed, parser_output);
    (node, errors)
}

pub(crate) fn build_tree(
    lexed: papyrus_parser::LexedStr<'_>,
    parser_output: papyrus_parser::Output,
) -> (GreenNode, Vec<SyntaxError>, bool) {
    let _p = tracing::info_span!("build_tree").entered();
    let mut builder = SyntaxTreeBuilder::default();

    let is_eof = lexed.intersperse_trivia(&parser_output, &mut |step| match step {
        papyrus_parser::StrStep::Token { kind, text } => builder.token(kind, text),
        papyrus_parser::StrStep::Enter { kind } => builder.start_node(kind),
        papyrus_parser::StrStep::Exit => builder.finish_node(),
        papyrus_parser::StrStep::Error { msg, pos } => {
            builder.error(msg.to_owned(), pos.try_into().unwrap())
        }
    });

    let (node, mut errors) = builder.finish_raw();
    for (i, err) in lexed.errors() {
        let text_range = lexed.text_range(i);
        let text_range = TextRange::new(
            text_range.start.try_into().unwrap(),
            text_range.end.try_into().unwrap(),
        );
        errors.push(SyntaxError::new(err, text_range))
    }

    (node, errors, is_eof)
}
