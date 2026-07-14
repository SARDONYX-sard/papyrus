//! Parse top-level items.

use self::{flags::flags, header::header, params, types};
use super::*;

pub(crate) fn source_file(p: &mut Parser<'_>) {
    let m = p.start();

    header(p);
    mod_contents(p);

    p.expect(EOF);

    m.complete(p, SourceFile);
}

fn mod_contents(p: &mut Parser<'_>) {
    while !p.at(EOF) {
        item(p);
    }
}

pub(crate) fn item(p: &mut Parser<'_>) {
    let m = p.start();

    match opt_item(p, m) {
        Ok(()) => {}
        Err(m) => {
            m.abandon(p);

            match p.current() {
                T![EndProperty] | T![EndState] | T![EndFunction] | T![EndEvent] => {
                    p.err_and_bump("unexpected block terminator");
                }
                _ => p.err_and_bump("expected item"),
            }
        }
    }
}

pub(super) const ITEM_RECOVERY_SET: TokenSet = TokenSet::new(&[
    T![Import],
    //
    T![Property],
    T![State],
    T![Function],
    T![Event],
    //
    T![EndProperty],
    T![EndState],
    T![EndFunction],
    T![EndEvent],
]);

fn opt_item(p: &mut Parser<'_>, m: Marker) -> Result<(), Marker> {
    match p.current() {
        T![Import] => import(p, m),
        T![State] | T![Auto] => state_stmt(p, m),
        T![Event] => event(p, m),
        T![Function] => function(p, m),

        _ if at_property(p) => property_stmt(p, m),
        _ if at_var_decl(p) => var_decl_stmt(p, m),

        _ => return Err(m),
    }

    Ok(())
}

fn import(p: &mut Parser<'_>, m: Marker) {
    p.bump(T![Import]);
    name_r(p, ITEM_RECOVERY_SET);

    m.complete(p, Import);
}

// int function foo()
// endFunction
fn function(p: &mut Parser<'_>, m: Marker) {
    opt_return_type(p);

    p.bump(T![Function]);

    name_r(p, ITEM_RECOVERY_SET);

    if p.at(T!['(']) {
        params::param_list(p);
    } else {
        p.error("expected function arguments");
    }

    flags(p);

    if !p.at(T![EndFunction]) {
        statements::block(p);
        p.expect(T![EndFunction]);
    }

    m.complete(p, Function);
}

fn event(p: &mut Parser<'_>, m: Marker) {
    p.expect(T![Event]);

    name(p);

    if p.at(T!['(']) {
        params::param_list(p);
    }

    flags(p);

    statements::block(p);

    p.expect(T![EndEvent]);

    m.complete(p, Event);
}

fn state_stmt(p: &mut Parser<'_>, m: Marker) {
    p.eat(T![Auto]);

    p.expect(T![State]);

    name(p);

    while !p.at(T![EndState]) && !p.at(EOF) {
        item(p);
    }

    p.expect(T![EndState]);

    m.complete(p, State);
}

/// Peek `<Type> "Property"`
fn at_property(p: &Parser<'_>) -> bool {
    if !types::at_type(p) {
        return false;
    }

    p.nth_at(1, T![Property])
}

fn property_stmt(p: &mut Parser<'_>, m: Marker) {
    types::ty(p);

    p.expect(T![Property]);

    name(p);

    if p.at(T![=]) {
        initializer(p);
        flags(p);

        m.complete(p, InlinePropertyStmt);
        return;
    }

    flags(p);

    property_members(p);

    p.expect(T![EndProperty]);

    m.complete(p, FullPropertyStmt);
}

fn property_members(p: &mut Parser<'_>) {
    while p.at(T![Function]) || types::at_type(p) {
        property_member(p);
    }
}

fn property_member(p: &mut Parser<'_>) {
    let m = p.start();
    let fn_m = p.start();
    function(p, fn_m);

    m.complete(p, PropertyMember);
}

fn at_var_decl(p: &Parser<'_>) -> bool {
    if !types::at_type(p) {
        return false;
    }

    p.nth_at(1, T![ident])
}

fn var_decl_stmt(p: &mut Parser<'_>, m: Marker) {
    types::ty(p);

    name(p);

    if p.at(T![=]) {
        initializer(p);
    }

    m.complete(p, VarDeclStmt);
}
