mod event;
mod lexed_str;
mod output;
mod shortcuts;
mod token_set;
#[macro_use]
mod syntax_kind;
mod grammar;
mod input;
mod parser;

#[cfg(test)]
mod tests;

pub use S_ as T;

pub(crate) use token_set::TokenSet;

pub use crate::{
    input::Input,
    lexed_str::LexedStr,
    output::{Output, Step},
    shortcuts::StrStep,
    syntax_kind::SyntaxKind,
};

#[derive(Debug, Clone, Copy)]
pub enum TopEntryPoint {
    SourceFile,
    Expr,
    Type,
}

impl TopEntryPoint {
    pub fn parse(self, input: &Input) -> Output {
        let entry: fn(&mut parser::Parser<'_>) = match self {
            TopEntryPoint::SourceFile => grammar::items::source_file,
            TopEntryPoint::Expr => grammar::expressions::expr,
            TopEntryPoint::Type => grammar::types::ty,
        };

        let mut p = parser::Parser::new(input);

        entry(&mut p);

        let (events, errors) = p.finish();
        let res = event::process(events, errors);

        if cfg!(debug_assertions) {
            let mut depth = 0;
            let mut first = true;
            for step in res.iter() {
                assert!(depth > 0 || first);
                first = false;
                match step {
                    Step::Enter { .. } => depth += 1,
                    Step::Exit => depth -= 1,
                    Step::FloatSplit { ends_in_dot: has_pseudo_dot } => {
                        depth -= 1 + !has_pseudo_dot as usize
                    }
                    Step::Token { .. } | Step::Error { .. } => (),
                }
            }
            assert!(!first, "no tree at all");
            assert_eq!(depth, 0, "unbalanced tree");
        }

        res
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PrefixEntryPoint {
    Item,
    Block,
    Stmt,
    Expr,
    Type,
}

impl PrefixEntryPoint {
    pub fn parse(self, input: &Input) -> Output {
        let entry: fn(&mut parser::Parser<'_>) = match self {
            PrefixEntryPoint::Item => grammar::items::item,
            PrefixEntryPoint::Block => grammar::statements::block,
            PrefixEntryPoint::Stmt => grammar::statements::stmt,
            PrefixEntryPoint::Expr => grammar::expressions::expr,
            PrefixEntryPoint::Type => grammar::types::ty,
        };

        let mut p = parser::Parser::new(input);

        entry(&mut p);

        let (events, errors) = p.finish();
        event::process(events, errors)
    }
}
