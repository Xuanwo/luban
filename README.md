# Luban

Luban is a standalone AI code editor app built with GPUI, with a Zed-like layout:

- Left: sidebar
- Center: timeline
- Right: diff / terminal

## Development

This project uses `just` to manage all common dev commands.

The embedded terminal requires Zig (0.14.1) for building `ghostty_vt_sys` (install via `brew install zig` or set `ZIG=/path/to/zig`).

To install a local Zig binary into `.context/zig/zig`:

```bash
just zig-bootstrap
```

```bash
just -l
```

### macOS Requirements

GPUI uses Metal shaders on macOS. Ensure the Metal toolchain component is installed:

```bash
xcodebuild -downloadComponent MetalToolchain
```

### Run

```bash
just run
```

### Codex SDK (local sidecar)

The Agent chat panel uses the `@openai/codex-sdk` TypeScript SDK via a small sidecar.

The bundled sidecar (`tools/codex_sidecar/dist/run.mjs`) is checked in. You generally should not rebuild it unless you bump the SDK version.

To rebuild the bundled script:

```bash
just sidecar-build
```

### Build

```bash
just build
```
