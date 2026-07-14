//! Parse types.

use super::*;

pub(super) fn at_type(p: &Parser<'_>) -> bool {
    matches!(p.current(), T![Int] | T![Float] | T![Bool] | T![String] | T![ident])
}

pub(crate) fn ty(p: &mut Parser<'_>) {
    assert!(at_type(p));

    let m = p.start();

    base_type(p);

    while p.at(T!['[']) {
        array_suffix(p);
    }

    m.complete(p, TYPE);
}

fn base_type(p: &mut Parser<'_>) {
    let m = p.start();

    match p.current() {
        T![Int] | T![Float] | T![Bool] | T![String] => primitive_type(p),

        T![ident] => custom_type(p),

        _ => {
            p.error("expected type");
        }
    }

    m.complete(p, BASE_TYPE);
}

fn primitive_type(p: &mut Parser<'_>) {
    let m = p.start();

    match p.current() {
        T![Int] | T![Float] | T![Bool] | T![String] => p.bump_any(),

        _ => p.error("expected primitive type"),
    }

    m.complete(p, PRIMITIVE_TYPE);
}

const TYPE_RECOVERY_SET: TokenSet = TokenSet::new(&[IDENT, T!['['], T![')'], T![,], T![=]]);

fn custom_type(p: &mut Parser<'_>) {
    let m = p.start();

    name_r(p, TYPE_RECOVERY_SET);

    m.complete(p, CUSTOM_TYPE);
}

fn array_suffix(p: &mut Parser<'_>) {
    let m = p.start();

    p.expect(T!['[']);
    p.expect(T![']']);

    m.complete(p, ARRAY_SUFFIX);
}
