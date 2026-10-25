ScriptName ParentBase extends ObjectReference
{The parent of ParentChild: its functions and events are called with `Parent.`.}

Event OnInit()
    Debug.Trace("ParentBase: init")
EndEvent

Function Greet(String asName)
    Debug.Trace("ParentBase: hello " + asName)
EndFunction

Int Function Count()
    Return 1
EndFunction
