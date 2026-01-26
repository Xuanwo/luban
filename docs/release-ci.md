# Release CI

This repository ships release artifacts via GitHub Actions when a tag is pushed.

## What the workflow does

Workflow file: `.github/workflows/release.yml`

- Trigger
  - `push` to tags matching `v*` (packages + uploads)
  - `pull_request` to `main` will run only when `justfile` or `.github/workflows/release.yml` changes (package test; no uploads)
- Outputs
  - Builds and uploads artifacts to Cloudflare R2 (used by `releases.luban.dev`)
  - Publishes a merged `latest.json` after all platform uploads succeed

Current targets:

- `darwin-universal`
- `linux-x86_64`
- `windows-x86_64`
- `linux-aarch64`
- `windows-aarch64`

## 1Password-based secrets

The workflow authenticates to 1Password using a service account token, then uses `op inject` to
materialize `.env` from `.env.example` before running `just` recipes.

### GitHub Secrets (required)

- `OP_SERVICE_ACCOUNT_TOKEN`
  - A 1Password service account token with access to the vault/items referenced below.
- `LUBAN_VAULT`
  - Vault name used by `.env.example`.

Note: `.env.example` contains `op://...` references and is the only place where secret reference
paths are configured.

### GitHub Variables (optional)
None.

## Artifacts directory

Packaging outputs are written under `dist/` (gitignored) by default.

## Releasing

Recommended: create and push a tag:

1. Ensure `LUBAN_VAULT` and `OP_SERVICE_ACCOUNT_TOKEN` are configured.
2. Create a tag like `v0.1.5` and push it (tags containing `-` are rejected).
3. Wait for the `Release` workflow to finish.
4. Verify `latest.json` and the uploaded artifacts on `releases.luban.dev`.
