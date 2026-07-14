use super::*;

pub(super) fn flags(p: &mut Parser<'_>) {
    while at_flag_modifier(p) {
        flag_modifier(p);
    }
}

fn flag_modifier(p: &mut Parser<'_>) {
    let m = p.start();

    match p.current() {
        T![Native] | T![Global] | T![Auto] | T![AutoReadOnly] | T![ident] => {
            p.bump_any();
        }
        _ => {
            p.error("expected flag modifier");
        }
    }

    m.complete(p, FLAG_MODIFIER);
}

fn at_flag_modifier(p: &Parser<'_>) -> bool {
    matches!(p.current(), T![Native] | T![Global] | T![Auto] | T![AutoReadOnly] | T![ident])
}
