`True` or `False`. Any value converts to `Bool` in conditions: `0`, `0.0`, `""` and `None` are
false.

# Examples

```papyrus
Bool isReady = False
If !isReady
    isReady = True
EndIf
```
