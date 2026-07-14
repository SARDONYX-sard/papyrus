//! This is a generated file, please do not edit manually. Changes can be
//! made in code generation that lives in `xtask` top-level dir.

#![allow(non_snake_case)]
use crate::{
    SyntaxKind::{self, *},
    SyntaxNode, SyntaxToken, T as S,
    ast::{AstChildren, AstNode, support},
};
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceFile {
    pub(crate) syntax: SyntaxNode,
}
impl SourceFile {
    pub fn script(&self) -> Option<Script> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Script {
    pub(crate) syntax: SyntaxNode,
}
impl Script {
    pub fn header(&self) -> Option<Header> { support::child(&self.syntax) }
    pub fn items(&self) -> AstChildren<Item> { support::children(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Header {
    pub(crate) syntax: SyntaxNode,
}
impl Header {
    pub fn script_name_decl(&self) -> Option<ScriptNameDecl> { support::child(&self.syntax) }
    pub fn extends_clause(&self) -> Option<ExtendsClause> { support::child(&self.syntax) }
    pub fn flag_modifiers(&self) -> AstChildren<FlagModifier> { support::children(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptNameDecl {
    pub(crate) syntax: SyntaxNode,
}
impl ScriptNameDecl {
    pub fn scriptname_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![ScriptName])
    }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtendsClause {
    pub(crate) syntax: SyntaxNode,
}
impl ExtendsClause {
    pub fn extends_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Extends]) }
    pub fn name_ref(&self) -> Option<NameRef> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FlagModifier {
    pub(crate) syntax: SyntaxNode,
}
impl FlagModifier {
    pub fn ident_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![ident]) }
    pub fn native_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Native]) }
    pub fn global_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Global]) }
    pub fn auto_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Auto]) }
    pub fn autoreadonly_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![AutoReadOnly])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    pub(crate) syntax: SyntaxNode,
}
impl Name {
    pub fn ident_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![ident]) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NameRef {
    pub(crate) syntax: SyntaxNode,
}
impl NameRef {
    pub fn ident_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![ident]) }
    pub fn self_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Self]) }
    pub fn parent_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Parent]) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Import {
    pub(crate) syntax: SyntaxNode,
}
impl Import {
    pub fn import_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Import]) }
    pub fn name_ref(&self) -> Option<NameRef> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarDeclStmt {
    pub(crate) syntax: SyntaxNode,
}
impl VarDeclStmt {
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn initializer(&self) -> Option<Initializer> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InlineProperty {
    pub(crate) syntax: SyntaxNode,
}
impl InlineProperty {
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
    pub fn property_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![Property])
    }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn initializer(&self) -> Option<Initializer> { support::child(&self.syntax) }
    pub fn flag_modifiers(&self) -> AstChildren<FlagModifier> { support::children(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FullProperty {
    pub(crate) syntax: SyntaxNode,
}
impl FullProperty {
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
    pub fn property_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![Property])
    }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn flag_modifiers(&self) -> AstChildren<FlagModifier> { support::children(&self.syntax) }
    pub fn property_member(&self) -> Option<PropertyMember> { support::child(&self.syntax) }
    pub fn property_members(&self) -> AstChildren<PropertyMember> {
        support::children(&self.syntax)
    }
    pub fn endproperty_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![EndProperty])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct State {
    pub(crate) syntax: SyntaxNode,
}
impl State {
    pub fn auto_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Auto]) }
    pub fn state_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![State]) }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn state_members(&self) -> AstChildren<StateMember> { support::children(&self.syntax) }
    pub fn endstate_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![EndState])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Function {
    pub(crate) syntax: SyntaxNode,
}
impl Function {
    pub fn return_type(&self) -> Option<ReturnType> { support::child(&self.syntax) }
    pub fn function_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![Function])
    }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn param_list(&self) -> Option<ParamList> { support::child(&self.syntax) }
    pub fn flag_modifiers(&self) -> AstChildren<FlagModifier> { support::children(&self.syntax) }
    pub fn body(&self) -> Option<Block> { support::child(&self.syntax) }
    pub fn endfunction_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![EndFunction])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Event {
    pub(crate) syntax: SyntaxNode,
}
impl Event {
    pub fn event_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Event]) }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn param_list(&self) -> Option<ParamList> { support::child(&self.syntax) }
    pub fn flag_modifiers(&self) -> AstChildren<FlagModifier> { support::children(&self.syntax) }
    pub fn body(&self) -> Option<Block> { support::child(&self.syntax) }
    pub fn endevent_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![EndEvent])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    pub(crate) syntax: SyntaxNode,
}
impl Type {
    pub fn base_type(&self) -> Option<BaseType> { support::child(&self.syntax) }
    pub fn array_suffixs(&self) -> AstChildren<ArraySuffix> { support::children(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArraySuffix {
    pub(crate) syntax: SyntaxNode,
}
impl ArraySuffix {
    pub fn l_brack_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S!['[']) }
    pub fn r_brack_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![']']) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrimitiveType {
    pub(crate) syntax: SyntaxNode,
}
impl PrimitiveType {
    pub fn int_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Int]) }
    pub fn float_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Float]) }
    pub fn bool_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Bool]) }
    pub fn string_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![String]) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomType {
    pub(crate) syntax: SyntaxNode,
}
impl CustomType {
    pub fn name_ref(&self) -> Option<NameRef> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParamList {
    pub(crate) syntax: SyntaxNode,
}
impl ParamList {
    pub fn l_paren_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S!['(']) }
    pub fn params(&self) -> AstChildren<Param> { support::children(&self.syntax) }
    pub fn r_paren_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![')']) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Param {
    pub(crate) syntax: SyntaxNode,
}
impl Param {
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
    pub fn name(&self) -> Option<Name> { support::child(&self.syntax) }
    pub fn initializer(&self) -> Option<Initializer> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Initializer {
    pub(crate) syntax: SyntaxNode,
}
impl Initializer {
    pub fn eq_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![=]) }
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PropertyMember {
    pub(crate) syntax: SyntaxNode,
}
impl PropertyMember {
    pub fn function(&self) -> Option<Function> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnType {
    pub(crate) syntax: SyntaxNode,
}
impl ReturnType {
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Block {
    pub(crate) syntax: SyntaxNode,
}
impl Block {
    pub fn stmts(&self) -> AstChildren<Stmt> { support::children(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssignStmt {
    pub(crate) syntax: SyntaxNode,
}
impl AssignStmt {}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnStmt {
    pub(crate) syntax: SyntaxNode,
}
impl ReturnStmt {
    pub fn return_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Return]) }
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IfStmt {
    pub(crate) syntax: SyntaxNode,
}
impl IfStmt {
    pub fn if_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![If]) }
    pub fn condition(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn elseif_branch(&self) -> AstChildren<ElseIfBranch> { support::children(&self.syntax) }
    pub fn endif_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![EndIf]) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileStmt {
    pub(crate) syntax: SyntaxNode,
}
impl WhileStmt {
    pub fn while_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![While]) }
    pub fn condition(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn loop_body(&self) -> Option<Block> { support::child(&self.syntax) }
    pub fn endwhile_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, S![EndWhile])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprStmt {
    pub(crate) syntax: SyntaxNode,
}
impl ExprStmt {
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElseIfBranch {
    pub(crate) syntax: SyntaxNode,
}
impl ElseIfBranch {
    pub fn elseif_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![ElseIf]) }
    pub fn condition(&self) -> Option<Expr> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElseBranch {
    pub(crate) syntax: SyntaxNode,
}
impl ElseBranch {
    pub fn else_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![Else]) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinExpr {
    pub(crate) syntax: SyntaxNode,
}
impl BinExpr {}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrefixExpr {
    pub(crate) syntax: SyntaxNode,
}
impl PrefixExpr {
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallExpr {
    pub(crate) syntax: SyntaxNode,
}
impl CallExpr {
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn arg_list(&self) -> Option<ArgList> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodCallExpr {
    pub(crate) syntax: SyntaxNode,
}
impl MethodCallExpr {
    pub fn receiver(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn dot_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![.]) }
    pub fn name_ref(&self) -> Option<NameRef> { support::child(&self.syntax) }
    pub fn arg_list(&self) -> Option<ArgList> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexExpr {
    pub(crate) syntax: SyntaxNode,
}
impl IndexExpr {
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn l_brack_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S!['[']) }
    pub fn r_brack_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![']']) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldExpr {
    pub(crate) syntax: SyntaxNode,
}
impl FieldExpr {
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn dot_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![.]) }
    pub fn name_ref(&self) -> Option<NameRef> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CastExpr {
    pub(crate) syntax: SyntaxNode,
}
impl CastExpr {
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn as_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![As]) }
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NewExpr {
    pub(crate) syntax: SyntaxNode,
}
impl NewExpr {
    pub fn new_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![New]) }
    pub fn ty(&self) -> Option<Type> { support::child(&self.syntax) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParenExpr {
    pub(crate) syntax: SyntaxNode,
}
impl ParenExpr {
    pub fn l_paren_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S!['(']) }
    pub fn expr(&self) -> Option<Expr> { support::child(&self.syntax) }
    pub fn r_paren_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![')']) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Literal {
    pub(crate) syntax: SyntaxNode,
}
impl Literal {}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArgList {
    pub(crate) syntax: SyntaxNode,
}
impl ArgList {
    pub fn l_paren_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S!['(']) }
    pub fn args(&self) -> AstChildren<Expr> { support::children(&self.syntax) }
    pub fn r_paren_token(&self) -> Option<SyntaxToken> { support::token(&self.syntax, S![')']) }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    Import(Import),
    VarDeclStmt(VarDeclStmt),
    InlineProperty(InlineProperty),
    FullProperty(FullProperty),
    State(State),
    Function(Function),
    Event(Event),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BaseType {
    PrimitiveType(PrimitiveType),
    CustomType(CustomType),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StateMember {
    Function(Function),
    Event(Event),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Stmt {
    VarDeclStmt(VarDeclStmt),
    AssignStmt(AssignStmt),
    ReturnStmt(ReturnStmt),
    IfStmt(IfStmt),
    WhileStmt(WhileStmt),
    ExprStmt(ExprStmt),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    BinExpr(BinExpr),
    PrefixExpr(PrefixExpr),
    CallExpr(CallExpr),
    MethodCallExpr(MethodCallExpr),
    IndexExpr(IndexExpr),
    FieldExpr(FieldExpr),
    CastExpr(CastExpr),
    NewExpr(NewExpr),
    ParenExpr(ParenExpr),
    Literal(Literal),
    NameRef(NameRef),
}
impl AstNode for SourceFile {
    fn can_cast(kind: SyntaxKind) -> bool { kind == SOURCE_FILE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Script {
    fn can_cast(kind: SyntaxKind) -> bool { kind == SCRIPT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Header {
    fn can_cast(kind: SyntaxKind) -> bool { kind == HEADER }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ScriptNameDecl {
    fn can_cast(kind: SyntaxKind) -> bool { kind == SCRIPT_NAME_DECL }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ExtendsClause {
    fn can_cast(kind: SyntaxKind) -> bool { kind == EXTENDS_CLAUSE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for FlagModifier {
    fn can_cast(kind: SyntaxKind) -> bool { kind == FLAG_MODIFIER }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Name {
    fn can_cast(kind: SyntaxKind) -> bool { kind == NAME }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for NameRef {
    fn can_cast(kind: SyntaxKind) -> bool { kind == NAME_REF }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Import {
    fn can_cast(kind: SyntaxKind) -> bool { kind == IMPORT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for VarDeclStmt {
    fn can_cast(kind: SyntaxKind) -> bool { kind == VAR_DECL_STMT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for InlineProperty {
    fn can_cast(kind: SyntaxKind) -> bool { kind == INLINE_PROPERTY }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for FullProperty {
    fn can_cast(kind: SyntaxKind) -> bool { kind == FULL_PROPERTY }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for State {
    fn can_cast(kind: SyntaxKind) -> bool { kind == STATE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Function {
    fn can_cast(kind: SyntaxKind) -> bool { kind == FUNCTION }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Event {
    fn can_cast(kind: SyntaxKind) -> bool { kind == EVENT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Type {
    fn can_cast(kind: SyntaxKind) -> bool { kind == TYPE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ArraySuffix {
    fn can_cast(kind: SyntaxKind) -> bool { kind == ARRAY_SUFFIX }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for PrimitiveType {
    fn can_cast(kind: SyntaxKind) -> bool { kind == PRIMITIVE_TYPE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for CustomType {
    fn can_cast(kind: SyntaxKind) -> bool { kind == CUSTOM_TYPE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ParamList {
    fn can_cast(kind: SyntaxKind) -> bool { kind == PARAM_LIST }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Param {
    fn can_cast(kind: SyntaxKind) -> bool { kind == PARAM }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Initializer {
    fn can_cast(kind: SyntaxKind) -> bool { kind == INITIALIZER }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for PropertyMember {
    fn can_cast(kind: SyntaxKind) -> bool { kind == PROPERTY_MEMBER }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ReturnType {
    fn can_cast(kind: SyntaxKind) -> bool { kind == RETURN_TYPE }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Block {
    fn can_cast(kind: SyntaxKind) -> bool { kind == BLOCK }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for AssignStmt {
    fn can_cast(kind: SyntaxKind) -> bool { kind == ASSIGN_STMT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ReturnStmt {
    fn can_cast(kind: SyntaxKind) -> bool { kind == RETURN_STMT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for IfStmt {
    fn can_cast(kind: SyntaxKind) -> bool { kind == IF_STMT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for WhileStmt {
    fn can_cast(kind: SyntaxKind) -> bool { kind == WHILE_STMT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ExprStmt {
    fn can_cast(kind: SyntaxKind) -> bool { kind == EXPR_STMT }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ElseIfBranch {
    fn can_cast(kind: SyntaxKind) -> bool { kind == ELSE_IF_BRANCH }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ElseBranch {
    fn can_cast(kind: SyntaxKind) -> bool { kind == ELSE_BRANCH }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for BinExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == BIN_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for PrefixExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == PREFIX_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for CallExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == CALL_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for MethodCallExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == METHOD_CALL_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for IndexExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == INDEX_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for FieldExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == FIELD_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for CastExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == CAST_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for NewExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == NEW_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ParenExpr {
    fn can_cast(kind: SyntaxKind) -> bool { kind == PAREN_EXPR }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for Literal {
    fn can_cast(kind: SyntaxKind) -> bool { kind == LITERAL }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl AstNode for ArgList {
    fn can_cast(kind: SyntaxKind) -> bool { kind == ARG_LIST }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) { Some(Self { syntax }) } else { None }
    }
    fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
impl From<Import> for Item {
    fn from(node: Import) -> Item { Item::Import(node) }
}
impl From<VarDeclStmt> for Item {
    fn from(node: VarDeclStmt) -> Item { Item::VarDeclStmt(node) }
}
impl From<InlineProperty> for Item {
    fn from(node: InlineProperty) -> Item { Item::InlineProperty(node) }
}
impl From<FullProperty> for Item {
    fn from(node: FullProperty) -> Item { Item::FullProperty(node) }
}
impl From<State> for Item {
    fn from(node: State) -> Item { Item::State(node) }
}
impl From<Function> for Item {
    fn from(node: Function) -> Item { Item::Function(node) }
}
impl From<Event> for Item {
    fn from(node: Event) -> Item { Item::Event(node) }
}
impl AstNode for Item {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            IMPORT | VAR_DECL_STMT | INLINE_PROPERTY | FULL_PROPERTY | STATE | FUNCTION | EVENT
        )
    }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        let res = match syntax.kind() {
            IMPORT => Item::Import(Import { syntax }),
            VAR_DECL_STMT => Item::VarDeclStmt(VarDeclStmt { syntax }),
            INLINE_PROPERTY => Item::InlineProperty(InlineProperty { syntax }),
            FULL_PROPERTY => Item::FullProperty(FullProperty { syntax }),
            STATE => Item::State(State { syntax }),
            FUNCTION => Item::Function(Function { syntax }),
            EVENT => Item::Event(Event { syntax }),
            _ => return None,
        };
        Some(res)
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            Item::Import(it) => it.syntax(),
            Item::VarDeclStmt(it) => it.syntax(),
            Item::InlineProperty(it) => it.syntax(),
            Item::FullProperty(it) => it.syntax(),
            Item::State(it) => it.syntax(),
            Item::Function(it) => it.syntax(),
            Item::Event(it) => it.syntax(),
        }
    }
}
impl From<PrimitiveType> for BaseType {
    fn from(node: PrimitiveType) -> BaseType { BaseType::PrimitiveType(node) }
}
impl From<CustomType> for BaseType {
    fn from(node: CustomType) -> BaseType { BaseType::CustomType(node) }
}
impl AstNode for BaseType {
    fn can_cast(kind: SyntaxKind) -> bool { matches!(kind, PRIMITIVE_TYPE | CUSTOM_TYPE) }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        let res = match syntax.kind() {
            PRIMITIVE_TYPE => BaseType::PrimitiveType(PrimitiveType { syntax }),
            CUSTOM_TYPE => BaseType::CustomType(CustomType { syntax }),
            _ => return None,
        };
        Some(res)
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            BaseType::PrimitiveType(it) => it.syntax(),
            BaseType::CustomType(it) => it.syntax(),
        }
    }
}
impl From<Function> for StateMember {
    fn from(node: Function) -> StateMember { StateMember::Function(node) }
}
impl From<Event> for StateMember {
    fn from(node: Event) -> StateMember { StateMember::Event(node) }
}
impl AstNode for StateMember {
    fn can_cast(kind: SyntaxKind) -> bool { matches!(kind, FUNCTION | EVENT) }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        let res = match syntax.kind() {
            FUNCTION => StateMember::Function(Function { syntax }),
            EVENT => StateMember::Event(Event { syntax }),
            _ => return None,
        };
        Some(res)
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            StateMember::Function(it) => it.syntax(),
            StateMember::Event(it) => it.syntax(),
        }
    }
}
impl From<VarDeclStmt> for Stmt {
    fn from(node: VarDeclStmt) -> Stmt { Stmt::VarDeclStmt(node) }
}
impl From<AssignStmt> for Stmt {
    fn from(node: AssignStmt) -> Stmt { Stmt::AssignStmt(node) }
}
impl From<ReturnStmt> for Stmt {
    fn from(node: ReturnStmt) -> Stmt { Stmt::ReturnStmt(node) }
}
impl From<IfStmt> for Stmt {
    fn from(node: IfStmt) -> Stmt { Stmt::IfStmt(node) }
}
impl From<WhileStmt> for Stmt {
    fn from(node: WhileStmt) -> Stmt { Stmt::WhileStmt(node) }
}
impl From<ExprStmt> for Stmt {
    fn from(node: ExprStmt) -> Stmt { Stmt::ExprStmt(node) }
}
impl AstNode for Stmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind, VAR_DECL_STMT | ASSIGN_STMT | RETURN_STMT | IF_STMT | WHILE_STMT | EXPR_STMT)
    }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        let res = match syntax.kind() {
            VAR_DECL_STMT => Stmt::VarDeclStmt(VarDeclStmt { syntax }),
            ASSIGN_STMT => Stmt::AssignStmt(AssignStmt { syntax }),
            RETURN_STMT => Stmt::ReturnStmt(ReturnStmt { syntax }),
            IF_STMT => Stmt::IfStmt(IfStmt { syntax }),
            WHILE_STMT => Stmt::WhileStmt(WhileStmt { syntax }),
            EXPR_STMT => Stmt::ExprStmt(ExprStmt { syntax }),
            _ => return None,
        };
        Some(res)
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            Stmt::VarDeclStmt(it) => it.syntax(),
            Stmt::AssignStmt(it) => it.syntax(),
            Stmt::ReturnStmt(it) => it.syntax(),
            Stmt::IfStmt(it) => it.syntax(),
            Stmt::WhileStmt(it) => it.syntax(),
            Stmt::ExprStmt(it) => it.syntax(),
        }
    }
}
impl From<BinExpr> for Expr {
    fn from(node: BinExpr) -> Expr { Expr::BinExpr(node) }
}
impl From<PrefixExpr> for Expr {
    fn from(node: PrefixExpr) -> Expr { Expr::PrefixExpr(node) }
}
impl From<CallExpr> for Expr {
    fn from(node: CallExpr) -> Expr { Expr::CallExpr(node) }
}
impl From<MethodCallExpr> for Expr {
    fn from(node: MethodCallExpr) -> Expr { Expr::MethodCallExpr(node) }
}
impl From<IndexExpr> for Expr {
    fn from(node: IndexExpr) -> Expr { Expr::IndexExpr(node) }
}
impl From<FieldExpr> for Expr {
    fn from(node: FieldExpr) -> Expr { Expr::FieldExpr(node) }
}
impl From<CastExpr> for Expr {
    fn from(node: CastExpr) -> Expr { Expr::CastExpr(node) }
}
impl From<NewExpr> for Expr {
    fn from(node: NewExpr) -> Expr { Expr::NewExpr(node) }
}
impl From<ParenExpr> for Expr {
    fn from(node: ParenExpr) -> Expr { Expr::ParenExpr(node) }
}
impl From<Literal> for Expr {
    fn from(node: Literal) -> Expr { Expr::Literal(node) }
}
impl From<NameRef> for Expr {
    fn from(node: NameRef) -> Expr { Expr::NameRef(node) }
}
impl AstNode for Expr {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            BIN_EXPR
                | PREFIX_EXPR
                | CALL_EXPR
                | METHOD_CALL_EXPR
                | INDEX_EXPR
                | FIELD_EXPR
                | CAST_EXPR
                | NEW_EXPR
                | PAREN_EXPR
                | LITERAL
                | NAME_REF
        )
    }
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        let res = match syntax.kind() {
            BIN_EXPR => Expr::BinExpr(BinExpr { syntax }),
            PREFIX_EXPR => Expr::PrefixExpr(PrefixExpr { syntax }),
            CALL_EXPR => Expr::CallExpr(CallExpr { syntax }),
            METHOD_CALL_EXPR => Expr::MethodCallExpr(MethodCallExpr { syntax }),
            INDEX_EXPR => Expr::IndexExpr(IndexExpr { syntax }),
            FIELD_EXPR => Expr::FieldExpr(FieldExpr { syntax }),
            CAST_EXPR => Expr::CastExpr(CastExpr { syntax }),
            NEW_EXPR => Expr::NewExpr(NewExpr { syntax }),
            PAREN_EXPR => Expr::ParenExpr(ParenExpr { syntax }),
            LITERAL => Expr::Literal(Literal { syntax }),
            NAME_REF => Expr::NameRef(NameRef { syntax }),
            _ => return None,
        };
        Some(res)
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            Expr::BinExpr(it) => it.syntax(),
            Expr::PrefixExpr(it) => it.syntax(),
            Expr::CallExpr(it) => it.syntax(),
            Expr::MethodCallExpr(it) => it.syntax(),
            Expr::IndexExpr(it) => it.syntax(),
            Expr::FieldExpr(it) => it.syntax(),
            Expr::CastExpr(it) => it.syntax(),
            Expr::NewExpr(it) => it.syntax(),
            Expr::ParenExpr(it) => it.syntax(),
            Expr::Literal(it) => it.syntax(),
            Expr::NameRef(it) => it.syntax(),
        }
    }
}
