# Fix Deploy Pipeline + Version Tracking

**Date:** 2026-04-05
**Branch:** safe-rust-cleanup

## Problem

CI deploys `www-cam/` artifacts (CAM-only JS + WASM) into `www/`, but `www/index.html` is the combined frontend with 4 modes (CAM, Sketch, Dataflow, Panel). The deployed site at `jvishnefske.github.io/cam/` is missing Dataflow and Panel functionality because:

1. `www/dist/main.js` comes from `www-cam/` build (CAM-only code)
2. `www/pkg/` only gets `rustcam.*` — missing `rustsim.*` WASM module
3. The Makefile has no target for building the combined `www/` frontend
4. The Makefile has an unresolved merge conflict on lines 76-96

Additionally, there is no way to verify what version is deployed — no build metadata is stamped.

## Changes

### 1. Makefile: Fix merge conflict + add targets

- Resolve merge conflict at lines 76-96: keep HEAD side (`serve-cam`, `serve-dataflow`, `serve-native`, `hil-e2e`)
- Add `ts-combined` target that:
  - Copies `www-cam/pkg/*` and `www-dataflow/pkg/*` into `www/pkg/`
  - Runs `cd www && npm ci && npm run typecheck && npm run test && npm run build`
- Add `ts-combined` to the `ts:` dependency list
- Add `version` target: writes `www/version.json` from git metadata
- Remove duplicate `verify` target (lines 63-67 duplicate 57-61)
- Remove duplicate `serve-native` (conflict residue)

### 2. CI workflow: Deploy combined frontend

In `.github/workflows/ci.yml`:

**Build job** — replace line 35:
```yaml
# OLD: mkdir -p www && cp -r www-cam/dist/* www/ && cp -r www-cam/pkg www/pkg
# NEW:
- run: make version
```
The `make ts` step (which now includes `ts-combined`) already builds `www/` with both WASM packages. The `make version` step stamps build metadata. No manual copy needed.

Wait — CI doesn't have git depth for SHA. Use `$GITHUB_SHA` and `$GITHUB_REF_NAME` env vars instead:
```yaml
- run: |
    echo '{"sha":"'${GITHUB_SHA::8}'","date":"'"$(date -u +%FT%TZ)"'","ref":"'"$GITHUB_REF_NAME"'"}' > www/version.json
```

**Build job** — update zip + artifact paths to use `www/` (already correct).

**deploy-external job** — same fix: replace `make wasm ts` + manual copy with `make wasm ts` (now includes combined) + version stamp. Remove the stale `mkdir -p www && cp -r ...` line.

### 3. Version tracking

**`www/version.json`** (generated, gitignored):
```json
{"sha":"abcd1234","date":"2026-04-05T22:40:00Z","ref":"main"}
```

**`www/src/version.ts`** — new module:
- Fetches `../version.json` on page load
- Logs to console: `RustCAM abcd1234 (2026-04-05)`
- Renders version string in the header bar (small dim text)
- Graceful fallback if version.json missing (local dev)

**`www/src/main.ts`** — import and call version init.

**`.gitignore`** — add `www/version.json`.

### 4. Files changed

| File | Action |
|------|--------|
| `Makefile` | Fix merge conflict, add `ts-combined` + `version` targets, fix duplicate `verify` |
| `.github/workflows/ci.yml` | Fix build assembly, add version stamp |
| `www/src/version.ts` | New: fetch + display version |
| `www/src/main.ts` | Import version module |
| `.gitignore` | Add `www/version.json` |
