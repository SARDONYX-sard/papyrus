Declares a state. Functions and events declared inside replace those of the empty state while the
script is in that state; switch with `GoToState("Name")`. Prefix with `Auto` for the initial state.

# Examples

```papyrus
Auto State Waiting
    Event OnActivate(ObjectReference akActionRef)
        GoToState("Busy")
    EndEvent
EndState

State Busy
EndState
```
