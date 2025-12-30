default:
  @just --list

sidecar-install:
  if [ ! -f crates/luban_app/agent_sidecar/node_modules/@openai/codex-sdk/package.json ]; then cd crates/luban_app/agent_sidecar && npm ci --no-fund --no-audit; fi

fmt:
  cargo fmt --all

lint:
  cargo clippy --workspace --all-targets --all-features -- -D warnings

test: sidecar-install
  cargo test --workspace --all-targets --all-features

test-fast:
  cargo test -p luban_domain

run: sidecar-install
  cargo run -p luban_app

build:
  cargo build -p luban_app

ci: fmt lint test
