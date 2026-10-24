Declares a function, with an optional return type before the keyword. The body ends with
`EndFunction`, except for `Native` functions which have no body.

# Examples

```papyrus
Int Function Add(Int a, Int b = 1) Global
    Return a + b
EndFunction

Function Kill() Native
```
