Veil 0.3.12

## Added

- Dashboard: request volume (20 min), alias mix, errors, average latency
- Upstream tabs: General (port 18787) plus one tab per app from `veil setup`
- Keys page: plain-language when to touch / when not to

## Removed

- Rules dry-run (UI and `/api/rules/dry-run`)

## Update

Console: Check for updates, or `veil update`.

If GitHub downloads fail:

```
gh release download v0.3.12 --repo Mouseww/veil -p veil-windows-x64.exe
```
