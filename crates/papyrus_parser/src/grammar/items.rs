//! Script level declarations.
//!
//! ```text
//! SourceFile   = NEWLINE* ScriptHeader? (Item | NEWLINE)*
//! ScriptHeader = 'ScriptName' Name ExtendsClause? Flag*
//! Item         = Import | VarDecl | Property | State | Function | Event
//! Import       = 'Import' NameRef
//! VarDecl      = Type Name Initializer? Flag*
//! Property     = Type 'Property' Name Initializer? Flag*
//!                (Function* 'EndProperty')?      ; omitted for `Auto` properties
//! State        = 'Auto'? 'State' Name Item* 'EndState'
//! Function     = Type? 'Function' Name ParamList Flag* (Block 'EndFunction')?
//! Event        = 'Event' Name ParamList Flag* (Block 'EndEvent')?
//! ParamList    = '(' (Param (',' Param)*)? ')'
//! Param        = Type Name Initializer?
//! ```
//!
//! Bodies are omitted for `Native` functions and events.

use super::{statements, *};
use crate::SyntaxKind;

pub(crate) fn source_file(p: &mut Parser<'_>) {
    let m = p.open();
    while p.eat(NEWLINE) {}

    if !p.at(SCRIPTNAME_KW) {
        p.error_expected("`ScriptName` declaration");
    }
    while !p.eof() {
        if !p.eat(NEWLINE) {
            item(p, false);
        }
    }
    p.close(m, SOURCE_FILE);
}

/// Parses one declaration. Always consumes at least one token.
fn item(p: &mut Parser<'_>, in_state: bool) {
    match p.current() {
        SCRIPTNAME_KW => script_header(p),
        IMPORT_KW => import(p),
        FUNCTION_KW => function(p),
        EVENT_KW => event(p),
        STATE_KW => state(p, in_state),
        AUTO_KW if p.nth_at(1, STATE_KW) => state(p, in_state),
        ENDIF_KW | ENDWHILE_KW | ENDFUNCTION_KW | ENDEVENT_KW | ENDPROPERTY_KW | ENDSTATE_KW
        | ELSE_KW | ELSEIF_KW => {
            let kw = p.current().describe();
            p.advance_with_error(format!("unexpected {kw} without a matching block"));
            eol(p);
        }
        _ => match type_len_at(p, 0).map(|n| p.nth(n)) {
            Some(FUNCTION_KW) => function(p),
            Some(PROPERTY_KW) => property(p),
            Some(IDENT) => var_decl(p),
            _ => {
                p.error_at_current(
                    "expected a declaration: `Import`, a variable, `Property`, `State`, `Function` or `Event`",
                );
                let m = p.open();
                p.advance();
                while !p.at_eol() {
                    p.advance();
                }
                p.close(m, ERROR);
            }
        },
    }
}

fn script_header(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance();
    name(p);
    if p.at(EXTENDS_KW) {
        let ext = p.open();
        p.advance();
        if p.at(IDENT) {
            name_ref(p);
        } else {
            p.error_expected("the name of the parent script");
        }
        p.close(ext, EXTENDS_CLAUSE);
    }
    flags(p);
    eol(p);
    p.close(m, SCRIPT_HEADER);
}

fn import(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance();
    if p.at(IDENT) {
        name_ref(p);
    } else {
        p.error_expected("the name of a script to import");
    }
    eol(p);
    p.close(m, IMPORT);
}

fn var_decl(p: &mut Parser<'_>) {
    let m = p.open();
    ty(p);
    name(p);
    if p.at(EQ) {
        initializer(p);
    }
    flags(p);
    eol(p);
    p.close(m, VAR_DECL);
}

fn property(p: &mut Parser<'_>) {
    let m = p.open();
    ty(p);
    p.advance(); // `Property`
    name(p);
    if p.at(EQ) {
        initializer(p);
    }
    let seen = flags(p);
    eol(p);

    if !seen.auto {
        loop {
            while p.eat(NEWLINE) {}
            let is_function =
                p.at(FUNCTION_KW) || type_len_at(p, 0).is_some_and(|n| p.nth_at(n, FUNCTION_KW));
            if !is_function {
                break;
            }
            function(p);
        }
        if p.eat(ENDPROPERTY_KW) {
            eol(p);
        } else {
            p.error_expected("`EndProperty` (or an `Auto` flag for an auto property)");
        }
    }
    p.close(m, PROPERTY);
}

fn state(p: &mut Parser<'_>, in_state: bool) {
    let m = p.open();
    if in_state {
        p.error_at_current("states cannot be nested");
    }
    p.eat(AUTO_KW);
    p.advance(); // `State`
    name(p);
    eol(p);

    loop {
        while p.eat(NEWLINE) {}
        let nested_state = p.at(STATE_KW) || (p.at(AUTO_KW) && p.nth_at(1, STATE_KW));
        if p.eof() || p.at(ENDSTATE_KW) || p.at(SCRIPTNAME_KW) || nested_state {
            break;
        }
        item(p, true);
    }
    if p.eat(ENDSTATE_KW) {
        eol(p);
    } else {
        p.error_expected("`EndState`");
    }
    p.close(m, STATE);
}

fn function(p: &mut Parser<'_>) {
    let m = p.open();
    if !p.at(FUNCTION_KW) {
        ty(p);
    }
    p.advance(); // `Function`
    callable_rest(p, ENDFUNCTION_KW);
    p.close(m, FUNCTION);
}

fn event(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance(); // `Event`
    callable_rest(p, ENDEVENT_KW);
    p.close(m, EVENT);
}

/// `Name ParamList Flag* (Block End)?` shared by functions and events.
fn callable_rest(p: &mut Parser<'_>, end: SyntaxKind) {
    name(p);
    if p.at(L_PAREN) {
        param_list(p);
    } else {
        p.error_expected("`(`");
    }
    let seen = flags(p);
    eol(p);
    if seen.native {
        return;
    }

    statements::block(p);
    if p.eat(end) {
        eol(p);
    } else {
        p.error_expected(end.describe());
    }
}

fn param_list(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance(); // `(`
    while !p.at(R_PAREN) && !p.at_eol() {
        if type_len_at(p, 0).is_some() {
            param(p);
        } else {
            p.advance_with_error("expected a parameter: `Type name`");
            continue;
        }
        if !p.eat(COMMA) && !p.at(R_PAREN) {
            p.error_expected("`,` or `)`");
            break;
        }
    }
    p.expect(R_PAREN);
    p.close(m, PARAM_LIST);
}

fn param(p: &mut Parser<'_>) {
    let m = p.open();
    ty(p);
    name(p);
    if p.at(EQ) {
        initializer(p);
    }
    p.close(m, PARAM);
}
