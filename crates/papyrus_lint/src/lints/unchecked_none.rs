//! `unchecked-cast` and `unchecked-call-result`: a value that may be `None`
//! used as a receiver without a check. Calling a function or reading a
//! property of `None` fails at run time: the call is skipped, its result is
//! `None` (or `0`, `False`...), and "Cannot call Kill() on a None object" is
//! logged.
//!
//! ```papyrus
//! Actor a = akTarget as Actor     ; None when akTarget is not an Actor
//! a.Kill()                        ; unchecked-cast (pedantic)
//!
//! ObjectReference r = GetLinkedRef()
//! r.Disable()                     ; unchecked-call-result (pedantic)
//! ```
//!
//! A check is a condition the use depends on:
//!
//! ```papyrus
//! If a                            ; or `a != None`
//!     a.Kill()
//! EndIf
//! If !a                           ; or `a == None`
//!     Return
//! EndIf
//! a.Kill()
//! If a && a.IsDead()
//! ```
//!
//! Casts to a parent type (`akActor as ObjectReference`) never fail and are
//! not reported. Only casts between scripts whose whole `extends` chain is
//! known are: when the imports of papyrus.toml provide every script, as for
//! compiling. Both lints are pedantic, allowed by default: many casts and calls
//! cannot give `None` in practice (`GetOwningQuest() as MyQuest`); turn them on in
//! `[lints]`. The
//! analysis is [`papyrus_ide::none_flow`], which hovers show too.

use papyrus_ide::{
    Diagnostic, TextRange,
    none_flow::{self, NoneSource},
    sema::Sema,
    typeck,
};

use crate::{Group, Lint, bodies};

pub(crate) const CAST: Lint = Lint {
    code: "unchecked-cast",
    group: Group::Pedantic,
    summary: "the result of a cast used without checking it is not None",
    run: run_cast,
    fix: None,
};

pub(crate) const CALL: Lint = Lint {
    code: "unchecked-call-result",
    group: Group::Pedantic,
    summary: "an object returned by a call used without checking it is not None",
    run: run_call,
    fix: None,
};

fn run_cast(sema: &Sema<'_>, out: &mut Vec<Diagnostic>) {
    run(sema, &CAST, out);
}

fn run_call(sema: &Sema<'_>, out: &mut Vec<Diagnostic>) {
    run(sema, &CALL, out);
}

fn run(sema: &Sema<'_>, lint: &'static Lint, out: &mut Vec<Diagnostic>) {
    let text =
        |r: TextRange| sema.text.get(usize::from(r.start())..usize::from(r.end())).unwrap_or("");
    for body in bodies(sema) {
        let types = typeck::check_body(sema, &body);
        for u in none_flow::analyze(sema, &body, &types).unchecked {
            let why = match (u.source, lint.code) {
                (NoneSource::Cast(at), "unchecked-cast") => {
                    format!("the cast `{}` gives None when it fails", text(at))
                }
                (NoneSource::Call(at), "unchecked-call-result") => {
                    format!("`{}` may return None", text(at))
                }
                _ => continue,
            };
            let (what, advice) = match u.local.map(|l| body.local(l).name) {
                Some(name) => (format!("`{name}`"), format!("check it first: `If {name}`")),
                None => {
                    ("this value".to_owned(), "keep it in a variable and check it first".to_owned())
                }
            };
            out.push(Diagnostic::warning(
                u.range,
                lint.code,
                format!("{what} may be None before `{}` ({why}): {advice}", u.member),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use papyrus_ide::Workspace;

    use crate::{Level, LintConfig};

    const BASE: &str = "ScriptName Form\nForm Function GetForm() Native\n";
    const REF: &str = "ScriptName ObjectReference extends Form\nFunction Disable() Native\nObjectReference Function GetLinkedRef() Native\n";
    const ACTOR: &str = "ScriptName Actor extends ObjectReference\nFunction Kill() Native\n";

    /// The problems of `code` in the body of `F(ObjectReference r, Actor b)`.
    fn check(code: &str, body: &str) -> Vec<String> {
        let src = format!(
            "ScriptName T extends ObjectReference\nFunction F(ObjectReference r, Actor b)\n{body}EndFunction\n"
        );
        let mut ws = Workspace::new();
        for (name, text) in [("Form", BASE), ("ObjectReference", REF), ("Actor", ACTOR)] {
            ws.set_open(PathBuf::from(format!("/ws/{name}.psc")), Arc::from(text), 0);
        }
        let file = ws.set_open(PathBuf::from("/ws/T.psc"), Arc::from(src.as_str()), 0);
        let mut config = LintConfig::default();
        config.set(code, Level::Warn).unwrap();
        crate::lint(&ws, file, &config)
            .into_iter()
            .filter(|d| d.code == code)
            .map(|d| {
                let line = src[..usize::from(d.range.start())].lines().count();
                format!(
                    "{line}: {}",
                    &src[usize::from(d.range.start())..usize::from(d.range.end())]
                )
            })
            .collect()
    }

    #[test]
    fn unchecked_casts() {
        let found = check(
            "unchecked-cast",
            "    Actor a = r as Actor\n    a.Kill()\n    (r as Actor).Kill()\n    (b as ObjectReference).Disable()\n",
        );
        assert_eq!(found, ["4: a", "5: r as Actor"]);
    }

    #[test]
    fn checks_silence() {
        let body = "    Actor a = r as Actor\n    If a\n        a.Kill()\n    EndIf\n    If a != None && a.IsDead()\n    EndIf\n    If !a\n        Return\n    EndIf\n    a.Kill()\n";
        assert_eq!(check("unchecked-cast", body), Vec::<String>::new());
        let elseif = "    Actor a = r as Actor\n    If a == None\n    ElseIf b\n        a.Kill()\n    EndIf\n";
        assert_eq!(check("unchecked-cast", elseif), Vec::<String>::new());
    }

    #[test]
    fn checks_end_with_their_branch() {
        let body =
            "    Actor a = r as Actor\n    If a\n        a.Kill()\n    EndIf\n    a.Kill()\n";
        assert_eq!(check("unchecked-cast", body), ["7: a"]);
        // Assigned again: unchecked again.
        let again = "    Actor a = r as Actor\n    If !a\n        Return\n    EndIf\n    a = r as Actor\n    a.Kill()\n";
        assert_eq!(check("unchecked-cast", again), ["8: a"]);
    }

    #[test]
    fn unchecked_call_results() {
        let body = "    ObjectReference x = GetLinkedRef()\n    x.Disable()\n    r.GetLinkedRef().Disable()\n";
        assert_eq!(check("unchecked-call-result", body), ["4: x", "5: r.GetLinkedRef()"]);
        // Allowed by default.
        assert!(crate::LintConfig::default().level(&super::CALL) == Level::Allow);
        assert!(crate::LintConfig::default().level(&super::CAST) == Level::Allow);
    }
}
