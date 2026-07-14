// SPDX-License-Identifier: Apache-2.0 OR MIT
// ref: https://github.com/apollographql/apollo-rs 8b64f55db8843e6f90e087e6bc77a91b8c45a537
//! Hand-written AST description for Papyrus.
//!
//! This file is consumed by xtask/codegen to generate the typed AST API.
//!
//! It intentionally does not depend on ungrammar.

pub(crate) struct KindsSrc {
    pub(crate) punct: &'static [(&'static str, &'static str)],
    pub(crate) keywords: &'static [&'static str],
    pub(crate) literals: &'static [&'static str],
    pub(crate) tokens: &'static [&'static str],
    pub(crate) nodes: &'static [&'static str],
}

pub(crate) const KINDS_SRC: KindsSrc = KindsSrc {
    punct: &[
        ("(", "L_PAREN"),
        (")", "R_PAREN"),
        ("[", "L_BRACK"),
        ("]", "R_BRACK"),
        (".", "DOT"),
        (",", "COMMA"),
        ("=", "EQ"),
        ("+=", "PLUS_EQ"),
        ("-=", "MINUS_EQ"),
        ("*=", "STAR_EQ"),
        ("/=", "SLASH_EQ"),
        ("%=", "PERCENT_EQ"),
        ("+", "PLUS"),
        ("-", "MINUS"),
        ("*", "STAR"),
        ("/", "SLASH"),
        ("%", "PERCENT"),
        ("<", "LT"),
        (">", "GT"),
        ("<=", "LTEQ"),
        (">=", "GTEQ"),
        ("==", "EQEQ"),
        ("!=", "NEQ"),
        ("&&", "AMP2"),
        ("||", "PIPE2"),
        ("&", "AMP"),
        ("|", "PIPE"),
        ("<<", "SHL"),
        (">>", "SHR"),
        ("!", "BANG"),
    ],

    // https://ck.uesp.net/wiki/Keyword_Reference
    keywords: &[
        "As",
        "Auto",
        "AutoReadOnly",
        "Bool",
        "Else",
        "ElseIf",
        "EndEvent",
        "EndFunction",
        "EndIf",
        "EndProperty",
        "EndState",
        "EndWhile",
        "Event",
        "Extends",
        "False",
        "Float",
        "Function",
        "Global",
        "If",
        "Import",
        "Int",
        "Length",
        "Native",
        "New",
        "None",
        "Parent",
        "Property",
        "Return",
        "ScriptName",
        "Self",
        "State",
        "String",
        "True",
        "While",
    ],

    literals: &["INT_NUMBER", "FLOAT_NUMBER", "STRING"],

    tokens: &["ERROR", "IDENT", "WHITESPACE", "NEWLINE", "COMMENT"],

    nodes: &[
        "SourceFile",
        "Script",
        "Header",
        "ScriptNameDecl",
        "ExtendsClause",
        "Import",
        "Name",
        "NameRef",
        "Type",
        "BaseType",
        "PrimitiveType",
        "CustomType",
        "ArraySuffix",
        "ParamList",
        "Param",
        "FlagModifier",
        "Property",
        "InlinePropertyStmt",
        "FullPropertyStmt",
        "PropertyMember",
        "State",
        "StateMember",
        "Function",
        "ReturnType",
        "Event",
        "Block",
        "Stmt",
        "VarDeclStmt",
        "Initializer",
        "AssignStmt",
        "ReturnStmt",
        "IfStmt",
        "ElseIfBranch",
        "ElseBranch",
        "WhileStmt",
        "ExprStmt",
        "Expr",
        "ParenExpr",
        "BinExpr",
        "PrefixExpr",
        "CallExpr",
        "ArgList",
        "MethodCallExpr",
        "FieldExpr",
        "IndexExpr",
        "CastExpr",
        "NewExpr",
        "Literal",
        "Ident",
    ],
};

// pub(crate) tokens is actually used once the code is generated.
#[allow(dead_code)]
#[derive(Default, Debug)]
pub(crate) struct CstSrc {
    pub(crate) tokens: Vec<String>,
    pub(crate) nodes: Vec<CstNodeSrc>,
    pub(crate) enums: Vec<CstEnumSrc>,
}

#[derive(Debug)]
pub(crate) struct CstNodeSrc {
    pub(crate) doc: Vec<String>,
    pub(crate) name: String,
    pub(crate) traits: Vec<String>,
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Field {
    Token(String),
    Node { name: String, ty: String, cardinality: Cardinality },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Cardinality {
    Optional,
    Many,
}

#[derive(Debug, Clone)]
pub(crate) struct CstEnumSrc {
    pub(crate) doc: Vec<String>,
    pub(crate) name: String,
    pub(crate) traits: Vec<String>,
    pub(crate) variants: Vec<String>,
}
