# lcity-citizen (compatibility shim)

`lcity-citizen` has been merged into **`lcity`**.

Use these commands instead:

```powershell
lcity citizen run
lcity citizen doctor
lcity citizen profile init --name default
lcity citizen tools preview
```

The `lcity-citizen` bin in this directory now just forwards to `lcity citizen ...` for local compatibility.

The real implementation lives in:

```text
lcity/
```
