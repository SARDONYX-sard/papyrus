#[cfg(test)]
mod tests {
    use papyrus_cst::{
        cst::{Tree, TreeKind},
        display_error::display_errors,
        parse_papyrus,
        parser::ParseError,
    };

    /// Assert there are no parse errors; print them if there are.
    fn assert_no_errors(src: &str, tree: &Tree, errors: &[ParseError]) {
        std::fs::write(
            "../../target/tokens.log",
            format!("{:#?}", papyrus_token::TokenStream::from(src).into_tokens()),
        )
        .unwrap();
        std::fs::write(
            "../../target/tree.log",
            papyrus_cst::debug::dump_tree_tokens_with_trivia(tree, src),
        )
        .unwrap();

        if !errors.is_empty() {
            let readable_errors = display_errors(src, "test.psc", errors);
            std::fs::write("../../target/error.log", &readable_errors).unwrap();
            panic!("unexpected parse errors in:\n{readable_errors}");
        }
    }

    // ── Smoke test: full fixture file ─────────────────────────────────────────

    #[test]
    fn full_fixture() {
        let src = include_str!("../../../test.psc");
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        assert_eq!(tree.kind, TreeKind::File);
    }

    // ── ScriptName ────────────────────────────────────────────────────────────

    #[test]
    fn scriptname_no_extends() {
        let src = "ScriptName Foo";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let script = tree.child_trees().next().expect("expected Script node");
        assert_eq!(script.kind, TreeKind::Script);
    }

    #[test]
    fn scriptname_with_extends() {
        let src = "ScriptName Foo Extends Bar";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let script = tree.child_trees().next().unwrap();
        assert_eq!(script.kind, TreeKind::Script);
        // Should have consumed all four tokens with no error.
        assert!(errors.is_empty());
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    #[test]
    fn empty_function() {
        let src = "Function Noop()\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        // Must have a ParamList and a Block child.
        assert!(
            func.find_child(TreeKind::ParamList).is_some(),
            "no ParamList"
        );
        assert!(func.find_child(TreeKind::Block).is_some(), "no Block");
    }

    #[test]
    fn function_with_return_type() {
        let src = "Int Function GetValue() \n    Return 42\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        let block = func.find_child(TreeKind::Block).unwrap();
        let ret = block
            .find_child(TreeKind::StmtReturn)
            .expect("no StmtReturn");
        assert!(
            ret.find_child(TreeKind::ExprLiteral).is_some(),
            "no literal in return"
        );
    }

    #[test]
    fn function_with_params() {
        let src = "Int Function Add(Int a, Int b)\n    Return a\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let params = func.find_child(TreeKind::ParamList).unwrap();
        let param_count = params.find_children(TreeKind::Param).count();
        assert_eq!(param_count, 2, "expected 2 params, got {param_count}");
    }

    #[test]
    fn function_with_default_param() {
        let src = "Function Greet(String name = \"World\")\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let params = func.find_child(TreeKind::ParamList).unwrap();
        assert!(params.find_child(TreeKind::Param).is_some());
    }

    #[test]
    fn native_global_function() {
        let src = "Function Log(String msg) Global Native";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        // Native functions have no Block.
        assert!(
            func.find_child(TreeKind::Block).is_none(),
            "native function must not have a Block"
        );
    }

    // ── Events ────────────────────────────────────────────────────────────────

    #[test]
    fn event_declaration() {
        let src = "Event OnActivate(ObjectReference akActionRef)\nEndEvent";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let ev = tree.child_trees().next().unwrap();
        assert_eq!(ev.kind, TreeKind::Event);
    }

    // ── Properties ────────────────────────────────────────────────────────────

    #[test]
    fn inline_auto_property() {
        let src = "bool property isOpen = false Auto conditional";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
    }

    #[test]
    fn auto_property() {
        let src = "Int Property Level Auto";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
    }

    #[test]
    fn full_property() {
        let src = "\
String Property Name
    String Function Get()
        Return \"\"
    EndFunction
    Function Set(String val)
    EndFunction
EndProperty";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
        let funcs: Vec<_> = prop.find_children(TreeKind::Function).collect();
        assert_eq!(funcs.len(), 2, "expected getter + setter");
    }

    // ── States ────────────────────────────────────────────────────────────────

    #[test]
    fn state_declaration() {
        let src = "State Busy\nEndState";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let s = tree.child_trees().next().unwrap();
        assert_eq!(s.kind, TreeKind::State);
    }

    #[test]
    fn state_with_event() {
        let src = "\
State Active
    Event OnBeginState()
    EndEvent
EndState";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let state = tree.child_trees().next().unwrap();
        let block = state.find_child(TreeKind::Block).unwrap();
        assert!(block.find_child(TreeKind::Event).is_some());
    }

    // ── Statements ────────────────────────────────────────────────────────────

    #[test]
    fn if_elseif_else() {
        let src = "\
Function F(Bool b)
    If b
        Return
    ElseIf b == False
        Return
    Else
        Return
    EndIf
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let if_node = block.find_child(TreeKind::StmtIf).expect("no StmtIf");
        assert!(
            if_node.find_child(TreeKind::StmtElseIf).is_some(),
            "no ElseIf"
        );
        assert!(if_node.find_child(TreeKind::StmtElse).is_some(), "no Else");
    }

    #[test]
    fn while_loop() {
        let src = "\
Function Count(Int n)
    Int i = 0
    While i < n
        i += 1
    EndWhile
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        assert!(block.find_child(TreeKind::StmtWhile).is_some());
    }

    #[test]
    fn return_no_value() {
        let src = "Function F()\n    Return\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let ret = block.find_child(TreeKind::StmtReturn).unwrap();
        // No expression child expected.
        assert!(ret.find_child(TreeKind::ExprLiteral).is_none());
        assert!(ret.find_child(TreeKind::ExprName).is_none());
    }

    #[test]
    fn var_decl_with_init() {
        let src = "Function F()\n    Int x = 42\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        // Variable declarations are wrapped in StmtExpr for now.
        assert!(block.find_child(TreeKind::StmtExpr).is_some());
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    #[test]
    fn binary_precedence_add_mul() {
        // `a + b * c` — multiplication must be nested under addition.
        let src = "Function F()\n    a + b * c\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        let outer = stmt
            .find_child(TreeKind::ExprBinary)
            .expect("no outer ExprBinary");
        assert!(
            outer.find_child(TreeKind::ExprBinary).is_some(),
            "inner ExprBinary (mul) should be nested under outer (add)"
        );
    }

    #[test]
    fn unary_negation() {
        let src = "Function F()\n    -x\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprUnary).is_some());
    }

    #[test]
    fn member_access() {
        let src = "Function F()\n    akActor.IsDead()\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(
            stmt.find_child(TreeKind::ExprCall).is_some(),
            "expected ExprCall"
        );
    }

    #[test]
    fn index_expression() {
        let src = "Function F()\n    arr[0]\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprIndex).is_some());
    }

    #[test]
    fn new_array() {
        let src = "Function F()\n    new Int[10]\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprLiteral).is_some());
    }

    #[test]
    fn cast_with_as() {
        let src = "Function F()\n    x as Int\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(
            stmt.find_child(TreeKind::ExprBinary).is_some(),
            "`as` should produce ExprBinary"
        );
    }

    #[test]
    fn self_and_parent() {
        let src = "Function F()\n    Self.Foo()\n    Parent.Bar()\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let calls: Vec<_> = block.find_children(TreeKind::StmtExpr).collect();
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn assignment_operators() {
        let src = "\
Function F()
    x = 1
    x += 2
    x -= 3
    x *= 4
    x /= 5
    x %= 6
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmts: Vec<_> = block.find_children(TreeKind::StmtExpr).collect();
        assert_eq!(stmts.len(), 6, "expected 6 assignment statements");
    }

    // ── Error recovery ────────────────────────────────────────────────────────

    #[test]
    fn missing_end_function_emits_error() {
        let src = "Function Broken()\n    Return\n; EndFunction intentionally missing";
        let (_tree, errors) = parse_papyrus(src);
        assert!(
            !errors.is_empty(),
            "expected at least one error for missing EndFunction"
        );
    }

    #[test]
    fn missing_endif_emits_error() {
        let src = "Function F()
If True
Return
; EndIf missing
EndFunction";
        let (_tree, errors) = parse_papyrus(src);
        assert!(!errors.is_empty(), "expected error for missing EndIf");
    }

    #[test]
    fn parser_recovers_after_error() {
        // The second function should still parse cleanly even though the first
        // is broken.
        let src = "\
Function Broken(
; missing RParen and EndFunction

Function Ok()
EndFunction";
        let (tree, _errors) = parse_papyrus(src);
        // We don't assert no errors, but the tree must be File-rooted and we
        // should find at least one Function node somewhere.
        assert_eq!(tree.kind, TreeKind::File);
        assert!(
            tree.child_trees().any(|c| c.kind == TreeKind::Function),
            "should have recovered and parsed at least one Function"
        );
    }
}
