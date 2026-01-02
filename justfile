default:
  @just --list

sidecar-build:
  mkdir -p tools/codex_sidecar/dist
  if [ -f tools/codex_sidecar/dist/run.mjs ]; then exit 0; fi
  cd tools/codex_sidecar && npm ci --no-fund --no-audit && npm run bundle

sidecar-install: sidecar-build

zig-bootstrap:
  bash tools/bootstrap-zig.sh

fmt:
  cargo fmt --all

lint:
  ZIG=$(pwd)/.context/zig/zig cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings

test:
  ZIG=$(pwd)/.context/zig/zig cargo test --workspace --all-targets --all-features

test-fast:
  ZIG=$(pwd)/.context/zig/zig cargo test -p luban_domain

run:
  ZIG=$(pwd)/.context/zig/zig cargo run -p luban_app

build:
  ZIG=$(pwd)/.context/zig/zig cargo build -p luban_app

ci: fmt lint test
