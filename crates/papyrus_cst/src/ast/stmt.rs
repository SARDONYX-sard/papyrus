use crate::ast::*;
use crate::{ast_enum, ast_node};

ast_node! {
    ExprStmt => StmtExpr {
        child expr: Expr;
    }
}

ast_node! {
    ReturnStmt => StmtReturn {
        child value: Expr;
    }
}

ast_node! {
    WhileStmt => StmtWhile {
        child condition: Expr;
        child body: Block;
    }
}

ast_node! {
    ElseStmt => StmtElse {
        child body: Block;
    }
}

ast_node! {
    ElseIfStmt => StmtElseIf {
        child condition: Expr;
        child body: Block;
    }
}

ast_node! {
    IfStmt => StmtIf {

        child condition: Expr;
        child body: Block;
        child else_clause: ElseStmt;

        children else_ifs: ElseIfStmt;
    }
}

ast_enum! {
    Statement {
        Expr(ExprStmt),
        Return(ReturnStmt),
        If(IfStmt),
        ElseIf(ElseIfStmt),
        Else(ElseStmt),
        While(WhileStmt),
    }
}

impl<'a> AstNode<'a> for Statement<'a> {
    const KIND: TreeKind = TreeKind::StmtExpr;

    fn syntax(&self) -> &'a Tree {
        match self {
            Self::Expr(t) => t.syntax(),
            Self::Return(t) => t.syntax(),
            Self::If(t) => t.syntax(),
            Self::ElseIf(t) => t.syntax(),
            Self::Else(t) => t.syntax(),
            Self::While(t) => t.syntax(),
        }
    }
}
