//! The [`SyntaxKind`] enum, shared by tokens and nodes of the syntax tree.

use papyrus_lexer::TokenKind;

macro_rules! syntax_kinds {
    (
        tokens { $($tok:ident = $lex:ident,)* }
        nodes { $($node:ident,)* }
    ) => {
        /// The kind of a syntax tree element: either a token produced by the lexer or a
        /// node produced by the parser.
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $(
                #[doc = concat!("Token produced from [`TokenKind::", stringify!($lex), "`].")]
                $tok,
            )*
            /// Virtual token marking the end of input. Never stored in a tree.
            EOF,
            $(
                #[doc = concat!("Node `", stringify!($node), "`.")]
                $node,
            )*
        }

        impl SyntaxKind {
            /// Converts a lexer token kind into the corresponding syntax kind.
            pub fn from_token(kind: TokenKind) -> SyntaxKind {
                match kind {
                    $(TokenKind::$lex => SyntaxKind::$tok,)*
                }
            }
        }
    };
}

syntax_kinds! {
    tokens {
        WHITESPACE = Whitespace,
        NEWLINE = Newline,
        LINE_COMMENT = LineComment,
        BLOCK_COMMENT = BlockComment,
        DOC_COMMENT = DocComment,
        LINE_CONTINUATION = LineContinuation,

        IDENT = Ident,
        INT_NUMBER = Int,
        FLOAT_NUMBER = Float,
        STRING = String,

        L_PAREN = LParen,
        R_PAREN = RParen,
        L_BRACK = LBracket,
        R_BRACK = RBracket,
        COMMA = Comma,
        DOT = Dot,
        EQ = Eq,
        EQ2 = EqEq,
        NEQ = NotEq,
        LT = Lt,
        LTEQ = LtEq,
        GT = Gt,
        GTEQ = GtEq,
        PLUS = Plus,
        PLUSEQ = PlusEq,
        MINUS = Minus,
        MINUSEQ = MinusEq,
        STAR = Star,
        STAREQ = StarEq,
        SLASH = Slash,
        SLASHEQ = SlashEq,
        PERCENT = Percent,
        PERCENTEQ = PercentEq,
        BANG = Bang,
        AMP2 = AmpAmp,
        PIPE2 = PipePipe,

        AS_KW = AsKw,
        AUTO_KW = AutoKw,
        AUTOREADONLY_KW = AutoReadOnlyKw,
        BOOL_KW = BoolKw,
        ELSE_KW = ElseKw,
        ELSEIF_KW = ElseIfKw,
        ENDEVENT_KW = EndEventKw,
        ENDFUNCTION_KW = EndFunctionKw,
        ENDIF_KW = EndIfKw,
        ENDPROPERTY_KW = EndPropertyKw,
        ENDSTATE_KW = EndStateKw,
        ENDWHILE_KW = EndWhileKw,
        EVENT_KW = EventKw,
        EXTENDS_KW = ExtendsKw,
        FALSE_KW = FalseKw,
        FLOAT_KW = FloatKw,
        FUNCTION_KW = FunctionKw,
        GLOBAL_KW = GlobalKw,
        IF_KW = IfKw,
        IMPORT_KW = ImportKw,
        INT_KW = IntKw,
        NATIVE_KW = NativeKw,
        NEW_KW = NewKw,
        NONE_KW = NoneKw,
        PARENT_KW = ParentKw,
        PROPERTY_KW = PropertyKw,
        RETURN_KW = ReturnKw,
        SCRIPTNAME_KW = ScriptNameKw,
        SELF_KW = SelfKw,
        STATE_KW = StateKw,
        STRING_KW = StringKw,
        TRUE_KW = TrueKw,
        WHILE_KW = WhileKw,

        ERROR_TOKEN = Unknown,
    }
    nodes {
        SOURCE_FILE,
        SCRIPT_HEADER,
        EXTENDS_CLAUSE,
        FLAG,
        IMPORT,
        VAR_DECL,
        PROPERTY,
        STATE,
        FUNCTION,
        EVENT,
        PARAM_LIST,
        PARAM,
        INITIALIZER,
        TYPE,
        NAME,
        NAME_REF,

        BLOCK,
        ASSIGN_STMT,
        EXPR_STMT,
        RETURN_STMT,
        IF_STMT,
        ELSE_IF_CLAUSE,
        ELSE_CLAUSE,
        WHILE_STMT,

        BIN_EXPR,
        PREFIX_EXPR,
        PAREN_EXPR,
        LITERAL,
        FIELD_EXPR,
        INDEX_EXPR,
        CALL_EXPR,
        ARG_LIST,
        ARG,
        CAST_EXPR,
        NEW_EXPR,

        ERROR,
    }
}

impl SyntaxKind {
    /// Returns `true` for whitespace, comments and line continuations.
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::WHITESPACE
                | SyntaxKind::LINE_COMMENT
                | SyntaxKind::BLOCK_COMMENT
                | SyntaxKind::DOC_COMMENT
                | SyntaxKind::LINE_CONTINUATION
        )
    }

    /// Returns `true` for comment tokens.
    pub fn is_comment(self) -> bool {
        matches!(
            self,
            SyntaxKind::LINE_COMMENT | SyntaxKind::BLOCK_COMMENT | SyntaxKind::DOC_COMMENT
        )
    }

    /// Returns `true` for reserved words.
    pub fn is_keyword(self) -> bool {
        self >= SyntaxKind::AS_KW && self <= SyntaxKind::WHILE_KW
    }

    /// Returns `true` for token kinds (as opposed to node kinds).
    pub fn is_token(self) -> bool {
        self < SyntaxKind::EOF
    }

    /// Returns `true` for the primitive type keywords `Bool`, `Float`, `Int` and `String`.
    pub fn is_primitive_type(self) -> bool {
        matches!(
            self,
            SyntaxKind::BOOL_KW | SyntaxKind::FLOAT_KW | SyntaxKind::INT_KW | SyntaxKind::STRING_KW
        )
    }

    /// Returns `true` for `=`, `+=`, `-=`, `*=`, `/=` and `%=`.
    pub fn is_assign_op(self) -> bool {
        matches!(
            self,
            SyntaxKind::EQ
                | SyntaxKind::PLUSEQ
                | SyntaxKind::MINUSEQ
                | SyntaxKind::STAREQ
                | SyntaxKind::SLASHEQ
                | SyntaxKind::PERCENTEQ
        )
    }

    /// A short human readable description, used in error messages.
    pub fn describe(self) -> &'static str {
        use SyntaxKind::*;
        match self {
            NEWLINE => "end of line",
            IDENT => "identifier",
            INT_NUMBER => "integer",
            FLOAT_NUMBER => "float",
            STRING => "string",
            L_PAREN => "`(`",
            R_PAREN => "`)`",
            L_BRACK => "`[`",
            R_BRACK => "`]`",
            COMMA => "`,`",
            DOT => "`.`",
            EQ => "`=`",
            AS_KW => "`As`",
            ELSE_KW => "`Else`",
            ENDEVENT_KW => "`EndEvent`",
            ENDFUNCTION_KW => "`EndFunction`",
            ENDIF_KW => "`EndIf`",
            ENDPROPERTY_KW => "`EndProperty`",
            ENDSTATE_KW => "`EndState`",
            ENDWHILE_KW => "`EndWhile`",
            EVENT_KW => "`Event`",
            FUNCTION_KW => "`Function`",
            PROPERTY_KW => "`Property`",
            SCRIPTNAME_KW => "`ScriptName`",
            STATE_KW => "`State`",
            EOF => "end of file",
            _ => "token",
        }
    }
}
