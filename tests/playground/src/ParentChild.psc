ScriptName ParentChild extends ParentBase
{Overrides of ParentBase. Hover `Parent`: the parent script, and the function it calls.}

Event OnInit()
    Parent.OnInit()
    Debug.Trace("ParentChild: init")
EndEvent

Function Greet(String asName)
    Parent.Greet(asName)
    Debug.Trace("ParentChild: hello " + asName)
EndFunction

Int Function Count()
    Return Parent.Count() + 1
EndFunction
