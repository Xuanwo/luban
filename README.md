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

The Agent chat panel uses the `@openai/codex-sdk` TypeScript SDK via a small sidecar.

Preferred: bundle the sidecar into a single script (so running the app does not require `npm install`):

```bash
just sidecar-build
```

### Build

```bash
just build
```
