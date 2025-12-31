default:
  @just --list

sidecar-build:
  mkdir -p tools/codex_sidecar/dist
  if [ -f tools/codex_sidecar/dist/run.mjs ]; then exit 0; fi
  cd tools/codex_sidecar && npm ci --no-fund --no-audit && npm run bundle

sidecar-install: sidecar-build

fmt:
  cargo fmt --all

lint:
  cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
  cargo test --workspace --all-targets --all-features

test-fast:
  cargo test -p luban_domain

run:
  cargo run -p luban_app

build:
  cargo build -p luban_app

ci: fmt lint test
