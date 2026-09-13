Veil 0.3.13

## Fixed

- Admin UI: all save operations (PUT /api/upstream, PUT /api/settings, etc.) returned 401
  after a fresh install because the auto-generated admin token was never delivered to the
  browser. The launcher now appends `?token=<token>` when opening the UI; the page reads
  and stores it on first load, then removes it from the address bar.
- Admin UI: token input field no longer resets to empty on page reload; it is now
  initialised from sessionStorage on mount.

## Update

Console: Check for updates, or `veil update`.

If GitHub downloads fail:

```
gh release download v0.3.13 --repo Mouseww/veil -p veil-windows-x64.exe
```
