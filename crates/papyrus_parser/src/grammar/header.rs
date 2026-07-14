use crate::grammar::{flags::flags, items::ITEM_RECOVERY_SET};

use super::*;

/// `"ScriptName" <ident> [extends <ident>`]
pub(super) fn header(p: &mut Parser<'_>) {
    let m = p.start();

    script_name_decl(p);

    if p.at(T![Extends]) {
        extends_clause(p);
    } else {
        p.error("expected flag modifier");
    }

    flags(p);

    m.complete(p, Header);
}

/// `"ScriptName" <ident> extends <ident>`
fn script_name_decl(p: &mut Parser<'_>) {
    let m = p.start();

    p.expect(T![ScriptName]);
    name(p);

    m.complete(p, ScriptNameDecl);
}

fn extends_clause(p: &mut Parser<'_>) {
    let m = p.start();

    p.expect(T![Extends]);
    name_r(p, ITEM_RECOVERY_SET);

    m.complete(p, ExtendsClause);
}
