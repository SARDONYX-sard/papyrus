A function without `Self`: it cannot use the properties and variables of the script and is called as
`ScriptName.Function()`.

# Examples

```papyrus
ScriptName MathUtil

Int Function Max(Int a, Int b) Global
    If a > b
        Return a
    EndIf
    Return b
EndFunction

; elsewhere: MathUtil.Max(1, 2)
```
