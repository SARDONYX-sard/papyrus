mod parsing;
mod ptr;
mod syntax_error;
mod syntax_node;
mod token_text;
mod validation;

pub mod ast;

use std::{marker::PhantomData, ops::Range};

use stdx::format_to;
use triomphe::Arc;

pub use crate::{
    ast::{AstNode, AstToken},
    ptr::{AstPtr, SyntaxNodePtr},
    syntax_error::SyntaxError,
    syntax_node::{
        PapyrusLanguage, PreorderWithTokens, SyntaxElement, SyntaxElementChildren, SyntaxNode,
        SyntaxNodeChildren, SyntaxToken, SyntaxTreeBuilder,
    },
    token_text::TokenText,
};
pub use papyrus_parser::{SyntaxKind, T};
pub use rowan::{
    Direction, GreenNode, NodeOrToken, SyntaxText, TextRange, TextSize, TokenAtOffset, WalkEvent,
    api::Preorder,
};
pub use rustc_literal_escaper as unescape;
pub use smol_str::{SmolStr, SmolStrBuilder, ToSmolStr, format_smolstr};

/// `Parse` is the result of the parsing: a syntax tree and a collection of
/// errors.
///
/// Note that we always produce a syntax tree, even for completely invalid
/// files.
#[derive(Debug, PartialEq, Eq)]
pub struct Parse<T> {
    green: Option<GreenNode>,
    errors: Option<Arc<[SyntaxError]>>,
    _ty: PhantomData<fn() -> T>,
}

impl<T> Clone for Parse<T> {
    fn clone(&self) -> Parse<T> {
        Parse { green: self.green.clone(), errors: self.errors.clone(), _ty: PhantomData }
    }
}

impl<T> Parse<T> {
    fn new(green: GreenNode, errors: Vec<SyntaxError>) -> Parse<T> {
        Parse {
            green: Some(green),
            errors: if errors.is_empty() { None } else { Some(errors.into()) },
            _ty: PhantomData,
        }
    }

    pub fn syntax_node(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.as_ref().unwrap().clone())
    }

    pub fn errors(&self) -> Vec<SyntaxError> {
        let mut errors = if let Some(e) = self.errors.as_deref() { e.to_vec() } else { vec![] };
        validation::validate(&self.syntax_node(), &mut errors);
        errors
    }
}

impl<T: AstNode> Parse<T> {
    /// Converts this parse result into a parse result for an untyped syntax tree.
    pub fn to_syntax(mut self) -> Parse<SyntaxNode> {
        let green = self.green.take();
        let errors = self.errors.take();
        Parse { green, errors, _ty: PhantomData }
    }

    /// Gets the parsed syntax tree as a typed ast node.
    ///
    /// # Panics
    ///
    /// Panics if the root node cannot be casted into the typed ast node
    /// (e.g. if it's an `ERROR` node).
    pub fn tree(&self) -> T {
        T::cast(self.syntax_node()).unwrap()
    }

    /// Converts from `Parse<T>` to [`Result<T, Vec<SyntaxError>>`].
    pub fn ok(self) -> Result<T, Vec<SyntaxError>> {
        match self.errors() {
            errors if !errors.is_empty() => Err(errors),
            _ => Ok(self.tree()),
        }
    }
}

impl Parse<SyntaxNode> {
    pub fn cast<N: AstNode>(mut self) -> Option<Parse<N>> {
        if N::cast(self.syntax_node()).is_some() {
            Some(Parse { green: self.green.take(), errors: self.errors.take(), _ty: PhantomData })
        } else {
            None
        }
    }
}

impl Parse<SourceFile> {
    pub fn debug_dump(&self) -> String {
        let mut buf = format!("{:#?}", self.tree().syntax());
        for err in self.errors() {
            format_to!(buf, "error {:?}: {}\n", err.range(), err);
        }
        buf
    }

    pub fn reparse(&self, delete: TextRange, insert: &str) -> Parse<SourceFile> {
        self.incremental_reparse(delete, insert)
            .unwrap_or_else(|| self.full_reparse(delete, insert))
    }

    fn incremental_reparse(&self, delete: TextRange, insert: &str) -> Option<Parse<SourceFile>> {
        // FIXME: validation errors are not handled here
        parsing::incremental_reparse(
            self.tree().syntax(),
            delete,
            insert,
            self.errors.as_deref().unwrap_or_default().iter().cloned(),
        )
        .map(|(green_node, errors, _reparsed_range)| Parse {
            green: Some(green_node),
            errors: if errors.is_empty() { None } else { Some(errors.into()) },
            _ty: PhantomData,
        })
    }

    fn full_reparse(&self, delete: TextRange, insert: &str) -> Parse<SourceFile> {
        let mut text = self.tree().syntax().text().to_string();
        text.replace_range(Range::<usize>::from(delete), insert);
        SourceFile::parse(&text)
    }
}

impl ast::Expr {
    /// Parses an `ast::Expr` from `text`.
    ///
    /// Note that if the parsed root node is not a valid expression, [`Parse::tree`] will panic.
    /// For example:
    /// ```rust,should_panic
    /// # use syntax::{ast, Edition};
    /// ast::Expr::parse("let fail = true;", Edition::CURRENT).tree();
    /// ```
    pub fn parse(text: &str) -> Parse<ast::Expr> {
        let _p = tracing::info_span!("Expr::parse").entered();
        let (green, errors) = parsing::parse_text_at(text, papyrus_parser::TopEntryPoint::Expr);
        let root = SyntaxNode::new_root(green.clone());

        assert!(
            ast::Expr::can_cast(root.kind()) || root.kind() == SyntaxKind::ERROR,
            "{:?} isn't an expression",
            root.kind()
        );
        Parse::new(green, errors)
    }
}

// #[cfg(not(no_salsa_async_drops))]
impl<T> Drop for Parse<T> {
    fn drop(&mut self) {
        let Some(green) = self.green.take() else {
            return;
        };
        static PARSE_DROP_THREAD: std::sync::OnceLock<std::sync::mpsc::Sender<GreenNode>> =
            std::sync::OnceLock::new();
        PARSE_DROP_THREAD
            .get_or_init(|| {
                let (sender, receiver) = std::sync::mpsc::channel::<GreenNode>();
                std::thread::Builder::new()
                    .name("ParseNodeDropper".to_owned())
                    .spawn(move || {
                        loop {
                            // block on a receive
                            _ = receiver.recv();
                            // then drain the entire channel
                            while receiver.try_recv().is_ok() {}
                            // and sleep for a bit
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        // why do this over just a `receiver.iter().for_each(drop)`? To reduce contention on the channel lock.
                        // otherwise this thread will constantly wake up and sleep again.
                    })
                    .unwrap();
                sender
            })
            .send(green)
            .unwrap();
    }
}

/// `SourceFile` represents a parse tree for a single Rust file.
pub use crate::ast::SourceFile;

impl SourceFile {
    pub fn parse(text: &str) -> Parse<SourceFile> {
        let _p = tracing::info_span!("SourceFile::parse").entered();
        let (green, errors) = parsing::parse_text(text);
        let root = SyntaxNode::new_root(green.clone());

        assert_eq!(root.kind(), SyntaxKind::SOURCE_FILE);
        Parse::new(green, errors)
    }
}

/// Matches a `SyntaxNode` against an `ast` type.
///
/// # Example:
///
/// ```ignore
/// match_ast! {
///     match node {
///         ast::CallExpr(it) => { ... },
///         ast::MethodCallExpr(it) => { ... },
///         ast::MacroCall(it) => { ... },
///         _ => None,
///     }
/// }
/// ```
#[macro_export]
macro_rules! match_ast {
    (match $node:ident { $($tt:tt)* }) => { $crate::match_ast!(match ($node) { $($tt)* }) };

    (match ($node:expr) {
        $( $( $path:ident )::+ ($it:pat) $(if $guard:expr)? => $res:expr, )*
        _ => $catch_all:expr $(,)?
    }) => {{
        #[allow(clippy::question_mark, reason = "if `$catch_all` is `return None` Clippy can mark this")]
        {
            $( if let Some($it) = $($path::)+cast($node.clone()) $(&& $guard)? { $res } else )*
            { $catch_all }
        }
    }};
}
