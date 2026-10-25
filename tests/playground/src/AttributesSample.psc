; #![allow(style)] inner attribute: the whole file
ScriptName AttributesSample extends ObjectReference
{Samples of attributes in comments: hover them, complete them after `(` or `,`.}

; #[allow(unused_local)] outer attribute: the whole function below
Function AllowedInFunction()
    Int notRead = 1
EndFunction

Function AllowedPerLine()
    Int a = 1 ; #[allow(unused_local)] after code: this line only
    ; #[allow(unused_local, suspicious)] alone on its line: the next statement
    Int b = 2
    Int c = 3 ; reported: `c` is never read
EndFunction

Function SkippedByTheFormatter()
    ; #[fmt::skip] aligned by hand, kept as written
    Int   width    =   10
    Debug.Trace("width = " + width)
EndFunction

; Warnings on the attributes themselves.
; #[allow(invalid_cast)] a compile error: cannot be allowed
; #[allow(unused_lokal)] unknown lint
; #[alow] unknown attribute
Function Mistakes()
EndFunction

Function Casts(ObjectReference akRef)
    Actor akActor = akRef as Actor
    ; hover `akActor`: may be None here (with `unchecked_cast` enabled: a warning)
    akActor.Kill()
    If akActor
        ; hover `akActor`: not None here
        akActor.Kill()
    EndIf
EndFunction
