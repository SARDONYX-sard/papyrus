//! Keyword classifier.

use crate::token::TokenKind;

#[inline]
pub fn classify(text: &str) -> TokenKind {
    match text {
        text if text.eq_ignore_ascii_case("as") => TokenKind::As,
        text if text.eq_ignore_ascii_case("auto") => TokenKind::Auto,
        text if text.eq_ignore_ascii_case("autoreadonly") => TokenKind::AutoReadOnly,
        text if text.eq_ignore_ascii_case("bool") => TokenKind::Bool,
        text if text.eq_ignore_ascii_case("conditional") => TokenKind::Conditional,
        text if text.eq_ignore_ascii_case("else") => TokenKind::Else,
        text if text.eq_ignore_ascii_case("elseif") => TokenKind::ElseIf,
        text if text.eq_ignore_ascii_case("endevent") => TokenKind::EndEvent,
        text if text.eq_ignore_ascii_case("endfunction") => TokenKind::EndFunction,
        text if text.eq_ignore_ascii_case("endif") => TokenKind::EndIf,
        text if text.eq_ignore_ascii_case("endproperty") => TokenKind::EndProperty,
        text if text.eq_ignore_ascii_case("endstate") => TokenKind::EndState,
        text if text.eq_ignore_ascii_case("endwhile") => TokenKind::EndWhile,
        text if text.eq_ignore_ascii_case("event") => TokenKind::Event,
        text if text.eq_ignore_ascii_case("extends") => TokenKind::Extends,
        text if text.eq_ignore_ascii_case("false") => TokenKind::False,
        text if text.eq_ignore_ascii_case("float") => TokenKind::Float,
        text if text.eq_ignore_ascii_case("function") => TokenKind::Function,
        text if text.eq_ignore_ascii_case("global") => TokenKind::Global,
        text if text.eq_ignore_ascii_case("hidden") => TokenKind::Hidden,
        text if text.eq_ignore_ascii_case("if") => TokenKind::If,
        text if text.eq_ignore_ascii_case("import") => TokenKind::Import,
        text if text.eq_ignore_ascii_case("int") => TokenKind::Int,
        text if text.eq_ignore_ascii_case("length") => TokenKind::Length,
        text if text.eq_ignore_ascii_case("native") => TokenKind::Native,
        text if text.eq_ignore_ascii_case("new") => TokenKind::New,
        text if text.eq_ignore_ascii_case("none") => TokenKind::None,
        text if text.eq_ignore_ascii_case("parent") => TokenKind::Parent,
        text if text.eq_ignore_ascii_case("property") => TokenKind::Property,
        text if text.eq_ignore_ascii_case("return") => TokenKind::Return,
        text if text.eq_ignore_ascii_case("scriptname") => TokenKind::ScriptName,
        text if text.eq_ignore_ascii_case("self") => TokenKind::Self_,
        text if text.eq_ignore_ascii_case("state") => TokenKind::State,
        text if text.eq_ignore_ascii_case("string") => TokenKind::StringTy,
        text if text.eq_ignore_ascii_case("true") => TokenKind::True,
        text if text.eq_ignore_ascii_case("while") => TokenKind::While,
        _ => TokenKind::Identifier,
    }
}

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeywordCase {
    #[default]
    Pascal,
    Lower,
    Preserve,
}

impl TokenKind {
    #[inline]
    pub fn keyword<'a>(&self, src: &'a str, case: KeywordCase) -> Option<Cow<'a, str>> {
        use KeywordCase::*;
        use TokenKind::*;

        let (pascal, lower) = match self {
            As => ("As", "as"),
            Auto => ("Auto", "auto"),
            AutoReadOnly => ("AutoReadOnly", "autoreadonly"),
            Bool => ("Bool", "bool"),
            Conditional => ("Conditional", "conditional"),
            Else => ("Else", "else"),
            ElseIf => ("ElseIf", "elseif"),
            EndEvent => ("EndEvent", "endevent"),
            EndFunction => ("EndFunction", "endfunction"),
            EndIf => ("EndIf", "endif"),
            EndProperty => ("EndProperty", "endproperty"),
            EndState => ("EndState", "endstate"),
            EndWhile => ("EndWhile", "endwhile"),
            Event => ("Event", "event"),
            Extends => ("Extends", "extends"),
            False => ("False", "false"),
            Float => ("Float", "float"),
            Function => ("Function", "function"),
            Global => ("Global", "global"),
            Hidden => ("Hidden", "hidden"),
            If => ("If", "if"),
            Import => ("Import", "import"),
            Int => ("Int", "int"),
            Length => ("Length", "length"),
            Native => ("Native", "native"),
            New => ("New", "new"),
            None => ("None", "none"),
            Parent => ("Parent", "parent"),
            Property => ("Property", "property"),
            Return => ("Return", "return"),
            ScriptName => ("ScriptName", "scriptname"),
            Self_ => ("Self", "self"),
            State => ("State", "state"),
            StringTy => ("String", "string"),
            True => ("True", "true"),
            While => ("While", "while"),
            _ => return Option::None,
        };

        Some(match case {
            Pascal => Cow::Borrowed(pascal),
            Lower => Cow::Borrowed(lower),
            Preserve => Cow::Borrowed(src),
        })
    }
}
