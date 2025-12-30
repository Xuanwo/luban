# Luban

Luban is a standalone AI code editor app built with GPUI, with a Zed-like layout:

- Left: sidebar
- Center: timeline
- Right: diff / terminal

## Development

This project uses `just` to manage all common dev commands.

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

The Agent chat panel uses the `@openai/codex-sdk` TypeScript SDK via a small Node.js sidecar.
`just run` and `just test` will install the sidecar dependencies automatically (requires Node.js 18+).

```bash
cd crates/luban_app/agent_sidecar
npm install
```

### Build

```bash
just build
```
