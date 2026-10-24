Calls the version of a function or event of the parent script, from an override.

# Examples

```papyrus
ScriptName MyActorScript extends Actor

Event OnDeath(Actor akKiller)
    Parent.OnDeath(akKiller)
    Debug.Trace("extra cleanup")
EndEvent
```
