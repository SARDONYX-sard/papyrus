Makes the script inherit the properties, functions and events of a parent script. Functions and
events of the parent can be overridden, and called with `Parent.Function()`.

# Examples

```papyrus
ScriptName MyActorScript extends Actor

Event OnDeath(Actor akKiller)
    Debug.Trace("died")
EndEvent
```
