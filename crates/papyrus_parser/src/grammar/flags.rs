use super::*;

pub(super) fn flags(p: &mut Parser<'_>) -> bool {
    let mut has_native = false;

    while at_flag_modifier(p) {
        has_native = flag_modifier(p);
    }

    has_native
}

fn flag_modifier(p: &mut Parser<'_>) -> bool {
    let mut has_native = false;

    let m = p.start();

    match p.current() {
        T![Native] => {
            p.bump_any();
            has_native = true;
        }
        T![Global] | T![Auto] | T![AutoReadOnly] | SyntaxKind::CUSTOM_FLAG => {
            p.bump_any();
        }
        invalid => p.error(format!("expected flag modifier. but got {invalid:?}")),
    }

    m.complete(p, FLAG_MODIFIER);
    has_native
}

fn at_flag_modifier(p: &Parser<'_>) -> bool {
    matches!(
        p.current(),
        T![Native] | T![Global] | T![Auto] | T![AutoReadOnly] | SyntaxKind::CUSTOM_FLAG
    )
}
