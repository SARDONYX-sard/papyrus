ScriptName CompilerBasics extends Quest Hidden
{ Arithmetic, casts, strings and control flow. }

Import Utility

Int Property Count = 3 Auto
Float Property Speed = 1.5 AutoReadOnly Hidden
Actor Property PlayerRef Auto
Int counter = 10
String label = "basics"
Bool enabled = True Conditional

Int Function Add(Int a, Int b = 2) Global
    Return a + b
EndFunction

Float Function Mix(Int i, Float f)
    Float r = i * f + i / 2 - i % 3
    r -= 1
    r *= 2.0
    Return -r
EndFunction

String Function Describe(Actor akActor, Int n)
    String s = "n=" + n + " ratio=" + (n as Float) / 3.0 + " ok=" + (n > 2)
    If akActor && n != 0 || !enabled
        s += " actor"
    ElseIf n >= 10
        s = s + " big"
    Else
        s = label
    EndIf
    Return s
EndFunction

Int Function Loop(Int limit)
    Int i = 0
    Int total = 0
    While i < limit && total <= 100
        total += Add(i, b = 3)
        i += 1
    EndWhile
    Return total
EndFunction

Function UseThings()
    Count = Count + 1
    counter -= Count
    Wait(0.5)
    Debug.Trace("count " + Count, 1)
    PlayerRef.Kill()
    Float mult = PlayerRef.GetActorValue("SpeedMult")
    Int[] values = new Int[5]
    values[0] = 4
    values[1] = values[0] * 2
    Int where = values.Find(8)
    Int size = values.Length
    Form f = PlayerRef as Form
    Actor a = f as Actor
    If a == None
        Return
    EndIf
    GoToState("Busy")
    String current = GetState()
EndFunction

Event OnInit()
    UseThings()
    RegisterForSingleUpdate(1.0)
EndEvent

State Busy
    Event OnUpdate()
        GoToState("")
    EndEvent

    Function UseThings()
        Debug.Notification("busy")
    EndFunction
EndState
