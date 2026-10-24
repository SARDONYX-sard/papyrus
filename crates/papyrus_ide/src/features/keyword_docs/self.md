The object this script is attached to. Not available in `Global` functions.

# Examples

```papyrus
ScriptName MyActivator extends ObjectReference

Event OnInit()
    Self.BlockActivation()
    (Self as ObjectReference).Disable()
EndEvent
```
