Leaves the function, with a value when the function declares a return type.

# Examples

```papyrus
Bool Function IsAdult(Actor akActor)
    If akActor.IsChild()
        Return False
    EndIf
    Return True
EndFunction
```
