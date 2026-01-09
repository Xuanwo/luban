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

web cmd profile="debug":
  if [ "{{cmd}}" = "build" ]; then \
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
    fi; \
  elif [ "{{cmd}}" = "run" ]; then \
    just web build "{{profile}}"; \
    if [ "{{profile}}" = "release" ]; then \
      cargo run -p luban_server --release; \
    elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
      cargo run -p luban_server; \
    else \
      cargo run -p luban_server --profile "{{profile}}"; \
    fi; \
  else \
    echo "usage: just web {build|run} [release]"; \
    exit 2; \
  fi

app cmd profile="debug":
  if [ "{{cmd}}" = "build" ]; then \
    just web build "{{profile}}"; \
    if [ "{{profile}}" = "release" ]; then \
      cargo build -p luban_tauri --release; \
    elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
      cargo build -p luban_tauri; \
    else \
      cargo build -p luban_tauri --profile "{{profile}}"; \
    fi; \
  elif [ "{{cmd}}" = "run" ]; then \
    just web build "{{profile}}"; \
    if [ "{{profile}}" = "release" ]; then \
      cargo run -p luban_tauri --release; \
    elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
      cargo run -p luban_tauri; \
    else \
      cargo run -p luban_tauri --profile "{{profile}}"; \
    fi; \
  else \
    echo "usage: just app {build|run} [release]"; \
    exit 2; \
  fi

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
  just web run "{{profile}}"

run-server profile="debug":
  if [ "{{profile}}" = "release" ]; then \
    cargo run -p luban_server --release; \
  elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
    cargo run -p luban_server; \
  else \
    cargo run -p luban_server --profile "{{profile}}"; \
  fi

build profile="debug":
  just app build "{{profile}}"

build-server profile="debug":
  if [ "{{profile}}" = "release" ]; then \
    cargo build -p luban_server --release; \
  elif [ "{{profile}}" = "debug" ] || [ "{{profile}}" = "dev" ]; then \
    cargo build -p luban_server; \
  else \
    cargo build -p luban_server --profile "{{profile}}"; \
  fi

ci: fmt lint test
