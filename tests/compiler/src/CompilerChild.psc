ScriptName CompilerChild extends CompilerBasics
{ Inheritance, parent calls and full properties. }

Int _level = 1

Int Property Level
    Int Function Get()
        Return _level
    EndFunction
    Function Set(Int value)
        If value < 0
            value = 0
        EndIf
        _level = value
    EndFunction
EndProperty

Bool Property Ready Hidden
    Bool Function Get()
        Return Level > 2
    EndFunction
EndProperty

Auto State Idle
    Event OnBeginState()
        Level = Level + 1
    EndEvent
EndState

Function UseThings()
    Parent.UseThings()
    CompilerBasics other = Self as CompilerBasics
    other.Count = 7
    Int n = other.Count
    Bool r = Ready
    Self.Level = n
    Debug.Trace(Mix(n, 2.0) as String)
EndFunction
