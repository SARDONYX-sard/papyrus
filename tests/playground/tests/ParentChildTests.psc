ScriptName ParentChildTests extends ParentChild
{Tests of ParentChild. Extending it, they call its functions on Self.
Run them with `papyrus_analyzer test`, or with ▶ Run test.}

; #[test]
Function CountAddsOne()
    Assert.IntEq(Count(), 2, "ParentBase.Count() + 1")
EndFunction

; #[test]
Function GreetCallsTheParent()
    Greet("Dovahkiin")
    Assert.IsTrue(Count() > 1)
EndFunction

; #[test]
Function FailsOnPurpose()
    Assert.IntEq(Count(), 3, "a failing test, to see the report: remove it")
EndFunction

; #[mock(Utility.RandomInt)] replaces the native in the tests of this script
Int Function FixedRoll(Int aiMin, Int aiMax)
    Return 4
EndFunction

; #[test]
Function RandomIsMocked()
    Assert.IntEq(Utility.RandomInt(1, 6), 4, "FixedRoll")
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
    Assert.IntEq(updates, 3)
EndFunction
