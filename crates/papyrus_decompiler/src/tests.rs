use std::{path::PathBuf, sync::Arc};

use expect_test::{Expect, expect};
use papyrus_compiler::CompileOptions;
use papyrus_ide::Workspace;
use papyrus_pex::{PexFile, normalize::normalize_with};

use crate::decompile;

fn compile(src: &str) -> PexFile<'static> {
    let mut ws = Workspace::new();
    let file = ws.set_open(PathBuf::from("/ws/T.psc"), Arc::from(src), 0);
    let pex = papyrus_compiler::compile(&ws, file, &CompileOptions::default())
        .unwrap_or_else(|e| panic!("{e:?}"));
    PexFile::from_bytes(&pex.to_bytes().unwrap()).unwrap().into_owned()
}

/// Decompiles the compiled `src`, and checks that the result compiles back to
/// the same code.
fn check(src: &str, expect: Expect) {
    let pex = compile(src);
    let text = decompile(&pex);
    expect.assert_eq(&text);
    let again = compile(&text);
    assert_eq!(normalize_with(&pex, true), normalize_with(&again, true), "{text}");
}

#[test]
fn roundtrip() {
    check(
        r#"ScriptName T Hidden
{ Docs. }
Int counter = 1
Int Property Count = 2 Auto
Float Property Rate = 1.5 AutoReadOnly
Int Property Full
    Int Function Get()
        Return counter
    EndFunction
    Function Set(Int value)
        counter = value
    EndFunction
EndProperty

Float Function Mix(Int i, Float f)
    Float r = i * f + (i as Float) / 2
    r -= 1
    Return -r
EndFunction

Function Run(Int n, Bool b)
    Int[] arr = new Int[5]
    While n > 0 && counter != 3 || b
        If n % 2 == 0
            Count += 1
        ElseIf n < 10
            arr[n] = Add(n, 2)
        Else
            Return
        EndIf
        n -= 1
    EndWhile
    String s = "n=" + n + " len=" + arr.Length
    Full = arr.Find(2)
EndFunction

Int Function Add(Int a, Int b) Global
    Return a + b
EndFunction

Auto State Busy
    Event OnBeginState()
        GoToState("")
    EndEvent
EndState
"#,
        expect![[r#"
            ScriptName T Hidden
            {Docs.}

            Int counter = 1

            Int Property Count = 2 Auto

            Float Property Rate = 1.5 AutoReadOnly

            Int Property Full
                Int Function Get()
                    Return counter
                EndFunction
                Function Set(Int value)
                    counter = value
                EndFunction
            EndProperty

            Float Function Mix(Int i, Float f)
                Float r = i * f + (i as Float) / 2
                r -= 1
                Return -r
            EndFunction

            Function Run(Int n, Bool b)
                Int[] arr = new Int[5]
                While n > 0 && counter != 3 || b
                    If n % 2 == 0
                        Count += 1
                    ElseIf n < 10
                        arr[n] = Add(n, 2)
                    Else
                        Return
                    EndIf
                    n -= 1
                EndWhile
                String s = "n=" + n + " len=" + arr.Length
                Full = arr.Find(2)
            EndFunction

            Int Function Add(Int a, Int b) Global
                Return a + b
            EndFunction

            Auto State Busy
                Event OnBeginState()
                    GoToState("")
                EndEvent
            EndState
        "#]],
    );
}
