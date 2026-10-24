Runs when none of the conditions of the `If` were true.

# Examples

```papyrus
If akActor.IsDead()
    Debug.Trace("dead")
Else
    akActor.Kill()
EndIf
```
