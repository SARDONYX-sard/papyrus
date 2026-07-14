use super::*;

const PARAM_FIRST: TokenSet = TokenSet::new(&[T![Int], T![Float], T![Bool], T![String], IDENT]);

pub(super) fn param_list(p: &mut Parser<'_>) {
    let m = p.start();

    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected parameter".to_owned(),
        PARAM_FIRST,
        |p| {
            param(p);
            true
        },
    );

    m.complete(p, PARAM_LIST);
}

fn param(p: &mut Parser<'_>) {
    let m = p.start();

    types::ty(p);

    name(p);

    if p.eat(T![=]) {
        expressions::expr(p);
    }

    m.complete(p, PARAM);
}

pub(super) fn arg_list(p: &mut Parser<'_>) {
    let m = p.start();

    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected call arguments".to_owned(),
        expressions::EXPR_FIRST,
        |p| {
            expressions::expr(p);
            true
        },
    );

    m.complete(p, ARG_LIST);
}
