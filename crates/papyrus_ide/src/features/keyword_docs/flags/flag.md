Defines a user flag: its name, its bit index (0 to 31) and optionally the declarations it is allowed
on. Without `{ }`, the flag is allowed everywhere.

# Examples

```papyrus
// Hides the script or property from the Creation Kit.
Flag Hidden 0
{
	Script
	Property
}

Flag Everywhere 5
```
