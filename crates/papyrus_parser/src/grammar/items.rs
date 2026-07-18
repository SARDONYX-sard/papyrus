//! Parse top-level items.

use self::{flags::flags, header::header, params, types};
use super::*;

pub(crate) fn source_file(p: &mut Parser<'_>) {
    let m = p.start();

    header(p);

    mod_contents(p);

    m.complete(p, SOURCE_FILE);
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
        T![Event] => event(p, m),
        T![State] | _ if at_has_auto_state(p) => state_stmt(p, m),
        T![Function] | _ if at_has_ret_function(p) => function(p, m),
        _ if at_property(p) => property(p, m),
        _ if at_var_decl(p) => var_decl_stmt(p, m),

        _ => return Err(m),
    }

    Ok(())
}

/// Peek current `Auto "State"`
fn at_has_auto_state(p: &Parser<'_>) -> bool {
    if p.current() != T![Auto] {
        return false;
    }
    p.nth_at(1, T![State])
}

/// Peek current `<Type> "Function"`
fn at_has_ret_function(p: &Parser<'_>) -> bool {
    if !types::at_type(p) {
        return false;
    }
    p.nth_at(1, T![Function])
}

/// Peek `<Type> "Property"`
fn at_property(p: &Parser<'_>) -> bool {
    if !types::at_type(p) {
        return false;
    }

    p.nth_at(1, T![Property])
}

/// Peek `<Type> <Ident> =`
fn at_var_decl(p: &Parser<'_>) -> bool {
    if !types::at_type(p) || !p.nth_at(1, T![ident]) {
        return false;
    }
    p.nth_at(2, T![=])
}

fn import(p: &mut Parser<'_>, m: Marker) {
    p.bump(T![Import]);
    name_r(p, ITEM_RECOVERY_SET);

    m.complete(p, IMPORT);
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

    let has_native = flags(p);
    if !has_native {
        statements::block(p);
        p.expect(T![EndFunction]);
    }

    if has_native && p.at(T![EndFunction]) {
        p.error("If the `native` flag is specified, should not have a fn body");
    }

    m.complete(p, FUNCTION);
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

    m.complete(p, EVENT);
}

fn state_stmt(p: &mut Parser<'_>, m: Marker) {
    p.eat(T![Auto]);

    p.expect(T![State]);

    name(p);

    while !p.at(T![EndState]) && !p.at(EOF) {
        item(p);
    }

    p.expect(T![EndState]);

    m.complete(p, STATE);
}

fn property(p: &mut Parser<'_>, m: Marker) {
    types::ty(p);

    p.expect(T![Property]);

    name(p);

    if p.at(T![=]) {
        initializer(p);
        flags(p);

        m.complete(p, INLINE_PROPERTY);
        return;
    }

    flags(p);

    property_members(p);

    p.expect(T![EndProperty]);

    m.complete(p, FULL_PROPERTY);
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

    m.complete(p, PROPERTY_MEMBER);
}

fn var_decl_stmt(p: &mut Parser<'_>, m: Marker) {
    types::ty(p);

    name(p);

    if p.at(T![=]) {
        initializer(p);
    }

    m.complete(p, VAR_DECL_STMT);
}
