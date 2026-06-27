/* Test! */
Scriptname TestPlayerScript extends Actor

Function OnInit()
    Debug.Trace("LSP test init")
EndFunction

; add a, b
Int Function AddNumbers(Int a, Int b) Global
    Return a + b
EndFunction

Function LoopTest()
    Int i = 0
    While i < 10
        Debug.Trace("i = " + i)
        i += 1
    EndWhile
EndFunction
