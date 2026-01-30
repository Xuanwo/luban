set dotenv-load

default:
  @just --list

fmt:
  cargo fmt --all

lint:
  cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings

test:
  sh -eu -c 'root="$(mktemp -d -t luban-test-XXXXXX)"; trap "rm -rf \"$root\"" EXIT; LUBAN_ROOT="$root" cargo test --workspace --all-targets --all-features; cargo test --manifest-path=dev/Cargo.toml'

test-fast:
  sh -eu -c 'root="$(mktemp -d -t luban-test-XXXXXX)"; trap "rm -rf \"$root\"" EXIT; LUBAN_ROOT="$root" cargo test -p luban_domain'

web cmd profile="debug":
  if [ "{{cmd}}" = "build" ]; then \
    if ! command -v pnpm >/dev/null 2>&1 && ! command -v pnpm.cmd >/dev/null 2>&1; then \
      echo "pnpm not found; install pnpm to build the web UI"; \
      exit 1; \
    fi; \
    if [ -d web ]; then \
      (cd web && pnpm install); \
      (cd web && \
        channel="$([ "{{profile}}" = "release" ] && echo release || echo dev)" && \
        tag="$( [ "$channel" = "release" ] && git describe --tags --exact-match 2>/dev/null || true )" && \
        version="${LUBAN_DISPLAY_VERSION:-${tag#v}}" && \
        version="${version:-$(node -p 'require("./package.json").version')}" && \
        NEXT_PUBLIC_LUBAN_VERSION="$version" \
        NEXT_PUBLIC_LUBAN_BUILD_CHANNEL="$channel" \
        NEXT_PUBLIC_LUBAN_COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)" \
        NEXT_PUBLIC_LUBAN_GIT_TAG="$tag" \
        NEXT_PUBLIC_LUBAN_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date)" \
        pnpm build); \
      mkdir -p web/out; \
      printf '\n' > web/out/.gitkeep; \
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
  elif [ "{{cmd}}" = "dev" ]; then \
    if ! command -v pnpm >/dev/null 2>&1 && ! command -v pnpm.cmd >/dev/null 2>&1; then \
      echo "pnpm not found; install pnpm to run the web dev server"; \
      exit 1; \
    fi; \
    if [ ! -d web/node_modules ]; then \
      (cd web && pnpm install); \
    fi; \
    (cd web && \
      channel="$([ "{{profile}}" = "release" ] && echo release || echo dev)" && \
      tag="$( [ "$channel" = "release" ] && git describe --tags --exact-match 2>/dev/null || true )" && \
      version="${LUBAN_DISPLAY_VERSION:-${tag#v}}" && \
      version="${version:-$(node -p 'require("./package.json").version')}" && \
      NEXT_PUBLIC_LUBAN_VERSION="$version" \
      NEXT_PUBLIC_LUBAN_BUILD_CHANNEL="$channel" \
      NEXT_PUBLIC_LUBAN_COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)" \
      NEXT_PUBLIC_LUBAN_GIT_TAG="$tag" \
      NEXT_PUBLIC_LUBAN_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date)" \
      pnpm dev); \
  elif [ "{{cmd}}" = "dev-mock" ]; then \
    if ! command -v pnpm >/dev/null 2>&1 && ! command -v pnpm.cmd >/dev/null 2>&1; then \
      echo "pnpm not found; install pnpm to run the web dev server"; \
      exit 1; \
    fi; \
    if [ ! -d web/node_modules ]; then \
      (cd web && pnpm install); \
    fi; \
    (cd web && \
      channel="$([ "{{profile}}" = "release" ] && echo release || echo dev)" && \
      tag="$( [ "$channel" = "release" ] && git describe --tags --exact-match 2>/dev/null || true )" && \
      version="${LUBAN_DISPLAY_VERSION:-${tag#v}}" && \
      version="${version:-$(node -p 'require("./package.json").version')}" && \
      NEXT_PUBLIC_LUBAN_MODE=mock \
      NEXT_PUBLIC_LUBAN_VERSION="$version" \
      NEXT_PUBLIC_LUBAN_BUILD_CHANNEL="$channel" \
      NEXT_PUBLIC_LUBAN_COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)" \
      NEXT_PUBLIC_LUBAN_GIT_TAG="$tag" \
      NEXT_PUBLIC_LUBAN_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date)" \
      pnpm dev); \
  else \
    echo "usage: just web {build|run|dev|dev-mock} [release]"; \
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
  if ! command -v pnpm >/dev/null 2>&1 && ! command -v pnpm.cmd >/dev/null 2>&1; then \
    echo "pnpm not found; cannot run UI tests"; \
    exit 1; \
  fi; \
  (cd web && pnpm test:ui)

test-ui-headed:
  if ! command -v pnpm >/dev/null 2>&1 && ! command -v pnpm.cmd >/dev/null 2>&1; then \
    echo "pnpm not found; cannot run UI tests"; \
    exit 1; \
  fi; \
  (cd web && LUBAN_AGENT_BROWSER_HEADED=1 pnpm test:ui)

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

package target profile="release" out_dir="dist":
  cargo run --quiet --manifest-path=dev/Cargo.toml -- package "{{target}}" --profile "{{profile}}" --out-dir "{{out_dir}}"

upload package_env="dist/package.env" latest="true":
  if [ ! -f "{{package_env}}" ]; then \
    echo "missing {{package_env}}; run: just package <target> [out_dir=...]"; \
    exit 1; \
  fi; \
  cargo run --quiet --manifest-path=dev/Cargo.toml -- upload --package-env "{{package_env}}" --latest "{{latest}}"

manifest packages_dir="dist" out="dist/latest.json":
  cargo run --quiet --manifest-path=dev/Cargo.toml -- manifest --packages-dir "{{packages_dir}}" --out "{{out}}"

upload-manifest manifest="dist/latest.json":
  cargo run --quiet --manifest-path=dev/Cargo.toml -- upload-manifest --manifest "{{manifest}}"
