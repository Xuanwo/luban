default:
  @just --list

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
