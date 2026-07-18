use crate::grammar::{flags::flags, items::ITEM_RECOVERY_SET};

use super::*;

/// `"ScriptName" <ident> [extends <ident>`]
pub(super) fn header(p: &mut Parser<'_>) {
    let m = p.start();

    script_name_decl(p);

    if p.at(T![Extends]) {
        extends_clause(p);
    }

    flags(p);

    m.complete(p, HEADER);
}

/// `"ScriptName" <ident>`
fn script_name_decl(p: &mut Parser<'_>) {
    let m = p.start();

    p.expect(T![ScriptName]);
    name(p);

    m.complete(p, SCRIPT_NAME_DECL);
}

fn extends_clause(p: &mut Parser<'_>) {
    let m = p.start();

    p.expect(T![Extends]);
    extend_clause(p, ITEM_RECOVERY_SET);

    m.complete(p, EXTENDS_CLAUSE);
}

/// define extends ident + err recovery
fn extend_clause(p: &mut Parser<'_>, recovery: TokenSet) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME);
    } else {
        p.err_recover("Expected a parent script name after `extends`", recovery);
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::*;

    #[test]
    fn header_minimal() {
        check(
            r#"ScriptName Foo"#,
            expect![[r#"
SOURCE_FILE
  HEADER
    SCRIPT_NAME_DECL
      ScriptName_KW "ScriptName"
      WHITESPACE " "
      NAME
        IDENT "Foo"
"#]],
        );
    }

    #[test]
    fn header_with_extends() {
        check(
            r#"
ScriptName Foo Extends Bar
"#,
            expect![[r#"
SOURCE_FILE
  NEWLINE "\n"
  HEADER
    SCRIPT_NAME_DECL
      ScriptName_KW "ScriptName"
      WHITESPACE " "
      NAME
        IDENT "Foo"
    WHITESPACE " "
    EXTENDS_CLAUSE
      Extends_KW "Extends"
      WHITESPACE " "
      NAME
        IDENT "Bar"
  NEWLINE "\n"
"#]],
        );
    }

    #[test]
    fn header_with_flags() {
        let custom_flags = &["hidden".into(), "conditional".into()];

        check_with_flags(
            r#"
ScriptName Foo Extends Bar Native Hidden Conditional
"#,
            expect![[r#"
SOURCE_FILE
  NEWLINE "\n"
  HEADER
    SCRIPT_NAME_DECL
      ScriptName_KW "ScriptName"
      WHITESPACE " "
      NAME
        IDENT "Foo"
    WHITESPACE " "
    EXTENDS_CLAUSE
      Extends_KW "Extends"
      WHITESPACE " "
      NAME
        IDENT "Bar"
    WHITESPACE " "
    FLAG_MODIFIER
      Native_KW "Native"
    WHITESPACE " "
    FLAG_MODIFIER
      CUSTOM_FLAG "Hidden"
    WHITESPACE " "
    FLAG_MODIFIER
      CUSTOM_FLAG "Conditional"
  NEWLINE "\n"
"#]],
            custom_flags,
        );
    }

    #[test]
    fn header_missing_script_name() {
        check_errors(
            r#"
Foo Extends Bar
"#,
            expect![[r#"
SOURCE_FILE
  NEWLINE "\n"
  HEADER
    SCRIPT_NAME_DECL
      NAME
        IDENT "Foo"
    WHITESPACE " "
    EXTENDS_CLAUSE
      Extends_KW "Extends"
      WHITESPACE " "
      NAME
        IDENT "Bar"
  NEWLINE "\n"
error 1: expected ScriptName_KW
"#]],
        );
    }

    #[test]
    fn header_missing_extends_name() {
        check_errors(
            r#"
ScriptName Foo Extends
"#,
            expect![[r#"
SOURCE_FILE
  NEWLINE "\n"
  HEADER
    SCRIPT_NAME_DECL
      ScriptName_KW "ScriptName"
      WHITESPACE " "
      NAME
        IDENT "Foo"
    WHITESPACE " "
    EXTENDS_CLAUSE
      Extends_KW "Extends"
      NEWLINE "\n"
      ERROR
error 24: Expected a parent script name after `extends`
"#]],
        );
    }

    #[test]
    fn header_duplicate_flags() {
        check(
            r#"
ScriptName Foo Extends Bar Native Native

int Function f() global
                return 1
endFunction
"#,
            expect![[r#"
SOURCE_FILE
  NEWLINE "\n"
  HEADER
    SCRIPT_NAME_DECL
      ScriptName_KW "ScriptName"
      WHITESPACE " "
      NAME
        IDENT "Foo"
    WHITESPACE " "
    EXTENDS_CLAUSE
      Extends_KW "Extends"
      WHITESPACE " "
      NAME
        IDENT "Bar"
    WHITESPACE " "
    FLAG_MODIFIER
      Native_KW "Native"
    WHITESPACE " "
    FLAG_MODIFIER
      Native_KW "Native"
  NEWLINE "\n"
  NEWLINE "\n"
  FUNCTION
    RETURN_TYPE
      TYPE
        BASE_TYPE
          PRIMITIVE_TYPE
            Int_KW "int"
    WHITESPACE " "
    Function_KW "Function"
    WHITESPACE " "
    NAME
      IDENT "f"
    PARAM_LIST
      L_PAREN "("
      R_PAREN ")"
    WHITESPACE " "
    FLAG_MODIFIER
      Global_KW "global"
    NEWLINE "\n"
    WHITESPACE "                "
    BLOCK
      RETURN_STMT
        Return_KW "return"
        WHITESPACE " "
        LITERAL
          INT_NUMBER "1"
    NEWLINE "\n"
    EndFunction_KW "endFunction"
  NEWLINE "\n"
"#]],
        );
    }
}
