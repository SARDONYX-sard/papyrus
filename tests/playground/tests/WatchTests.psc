ScriptName WatchTests
{To try the debugger: click ⚙ Debug above a test, with a breakpoint on a
line marked `watch:`, then add the expressions of the comment to WATCH (or
hover a name, or type them in the Debug Console).}
Import PapyrusAnalyzer

String Property Label = "watch" Auto
Int Property Count = 0 Auto

; #[test]
Function SumsAnArray()
    Int[] values = New Int[4]
    Int i = 0
    While i < values.Length
        values[i] = (i + 1) * 10
        i += 1
    EndWhile
    Int total = 0
    i = 0
    While i < values.Length
        total += values[i] ; watch: total, i, values[i] * 2, values.Length, total >= 30 && i < 3
        i += 1
    EndWhile
    Count = total
    AssertIntEq(total, 100) ; watch: Count, Self.Count / 4, Label + "!", values
EndFunction

; #[test]
Function ScalesAFloat()
    Float half = 0.5
    Float scaled = Scale(Count + 4, half)
    AssertFloatEq(scaled, 2.0)
EndFunction

Float Function Scale(Int aiValue, Float afBy)
    Return aiValue * afBy ; watch: aiValue * afBy, (aiValue As String) + "x", afBy As Int
EndFunction
