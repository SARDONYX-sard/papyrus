use crate::ast::*;
use crate::{ast_enum, ast_node};

ast_node! {
    LiteralExpr => ExprLiteral {
    }
}

ast_node! {
    ParenExpr => ExprParen {
        child expr: Expr;
    }
}

ast_node! {
    UnaryExpr => ExprUnary {
        child expr: Expr;
    }
}

ast_node! {
    BinaryExpr => ExprBinary {
    }
}

ast_node! {
    CallExpr => ExprCall {
        child args: ArgList;
    }
}

ast_node! {
    MemberExpr => ExprMember {
    }
}

ast_node! {
    IndexExpr => ExprIndex {
        child index: Expr;
    }
}

ast_node! {
    ArgList => ArgList {
        children args: Arg;
    }
}

ast_node! {
    Arg => Arg {
        child expr: Expr;
    }
}

ast_enum! {
    Expr {
        Name(Name),
        Literal(LiteralExpr),
        Paren(ParenExpr),
        Unary(UnaryExpr),
        Binary(BinaryExpr),
        Call(CallExpr),
        Member(MemberExpr),
        Index(IndexExpr),
    }
}

impl<'a> AstNode<'a> for Expr<'a> {
    const KIND: TreeKind = TreeKind::StmtExpr;

    fn syntax(&self) -> &'a Tree {
        match self {
            Expr::Name(name) => name.syntax(),
            Expr::Literal(literal_expr) => literal_expr.syntax(),
            Expr::Paren(paren_expr) => paren_expr.syntax(),
            Expr::Unary(unary_expr) => unary_expr.syntax(),
            Expr::Binary(binary_expr) => binary_expr.syntax(),
            Expr::Call(call_expr) => call_expr.syntax(),
            Expr::Member(member_expr) => member_expr.syntax(),
            Expr::Index(index_expr) => index_expr.syntax(),
        }
    }
}
