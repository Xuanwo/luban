# Luban

Luban is a standalone AI code editor app built with GPUI, with a Zed-like layout:

- Left: sidebar
- Center: timeline
- Right: diff / terminal

## Development

### macOS Requirements

GPUI uses Metal shaders on macOS. Ensure the Metal toolchain component is installed:

```bash
xcodebuild -downloadComponent MetalToolchain
```

### Run

```bash
cargo run -p luban_app
```

### Build

```bash
cargo build -p luban_app
```
