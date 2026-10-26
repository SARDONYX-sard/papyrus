ScriptName ParentChildTests extends ParentChild
{Tests of ParentChild. Extending it, they call its functions on Self.
Run them with `papyrus_analyzer test`, or with ▶ Run test.}
Import PapyrusAnalyzer

; #[test]
Function CountAddsOne()
    AssertIntEq(Count(), 2, "ParentBase.Count() + 1")
EndFunction

; #[test]
Function GreetCallsTheParent()
    Greet("Dovahkiin")
    AssertTrue(Count() > 1)
EndFunction

; #[test]
Function FailsOnPurpose()
    AssertIntEq(Count(), 3, "a failing test, to see the report: remove it")
EndFunction

; #[mock(Utility.RandomInt)] replaces the native in the tests of this script
Int Function FixedRoll(Int aiMin, Int aiMax)
    Return 4
EndFunction

; #[test]
Function RandomIsMocked()
    AssertIntEq(Utility.RandomInt(1, 6), 4, "FixedRoll")
EndFunction

Int updates

Event OnUpdate()
    updates += 1
EndEvent

; #[test] Utility.Wait sends the OnUpdate events that are due
Function WaitSendsUpdates()
    RegisterForUpdate(1.0)
    Utility.Wait(3.5)
    UnregisterForUpdate()
    Utility.Wait(5.0)
    AssertIntEq(updates, 3)
EndFunction
