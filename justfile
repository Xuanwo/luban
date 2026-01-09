default:
  @just --list

zig-bootstrap:
  bash tools/bootstrap-zig.sh

fmt:
  cargo fmt --all

lint:
  cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings

test:
  cargo test --workspace --all-targets --all-features

test-fast:
  cargo test -p luban_domain

test-ui:
  if command -v pnpm >/dev/null 2>&1; then \
    (cd web && pnpm test:e2e); \
  elif command -v npm >/dev/null 2>&1; then \
    (cd web && npm run test:e2e); \
  else \
    echo "pnpm/npm not found; cannot run Playwright tests"; \
    exit 1; \
  fi

test-ui-headed:
  if command -v pnpm >/dev/null 2>&1; then \
    (cd web && pnpm test:e2e:headed); \
  elif command -v npm >/dev/null 2>&1; then \
    (cd web && npm run test:e2e:headed); \
  else \
    echo "pnpm/npm not found; cannot run Playwright tests"; \
    exit 1; \
  fi

run profile="debug":
  if command -v pnpm >/dev/null 2>&1; then \
    if [ -d web ]; then \
      (cd web && pnpm install); \
      if [ -f web/node_modules/ghostty-web/ghostty-vt.wasm ]; then \
        mkdir -p web/public; \
        cp web/node_modules/ghostty-web/ghostty-vt.wasm web/public/ghostty-vt.wasm; \
      fi; \
      (cd web && pnpm build); \
    fi; \
  elif command -v npm >/dev/null 2>&1; then \
    if [ -d web ]; then \
      if [ ! -d web/node_modules ]; then \
        (cd web && npm install); \
      fi; \
      if [ -f web/node_modules/ghostty-web/ghostty-vt.wasm ]; then \
        mkdir -p web/public; \
        cp web/node_modules/ghostty-web/ghostty-vt.wasm web/public/ghostty-vt.wasm; \
      fi; \
      (cd web && npm run build); \
    fi; \
  else \
    echo "pnpm/npm not found; skipping web build"; \
  fi
  if [ "{{profile}}" = "release" ]; then \
    cargo run -p luban_server --release; \
  elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
    cargo run -p luban_server; \
  else \
    cargo run -p luban_server --profile "{{profile}}"; \
  fi

run-server profile="debug":
  if [ "{{profile}}" = "release" ]; then \
    cargo run -p luban_server --release; \
  elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
    cargo run -p luban_server; \
  else \
    cargo run -p luban_server --profile "{{profile}}"; \
  fi

build profile="debug":
  if [ "{{profile}}" = "release" ]; then \
    cargo build -p luban_app --release; \
  elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
    cargo build -p luban_app; \
  else \
    cargo build -p luban_app --profile "{{profile}}"; \
  fi

build-server profile="debug":
  if [ "{{profile}}" = "release" ]; then \
    cargo build -p luban_server --release; \
  elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
    cargo build -p luban_server; \
  else \
    cargo build -p luban_server --profile "{{profile}}"; \
  fi

ci: fmt lint test
