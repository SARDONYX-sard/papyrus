;/ script docs /;
ScriptName TestPlayerScript extends Actor

;/ event docs /;
Event OnInit()
    Debug.Trace("LSP test init")
EndEvent

; fn docs: add a, b
Int Function AddNumbers(Int a, Int b) Global
    Return a + b ; add a + b(expected same line comment)
EndFunction

Function LoopTest()
    ; var docs
    Int i = 0
    While i < 10
        Debug.Trace("i = " + i)
        i += 1
    EndWhile
EndFunction
