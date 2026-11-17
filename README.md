<img src="app/src-tauri/icons/128x128@2x.png" alt="PhantomPOS icon" width="96" height="96">

# PhantomPOS

PhantomPOS is a virtual ESC/POS thermal printer. It is a desktop application that
listens on TCP port 9100, interprets ESC/POS commands and shows on screen the receipt
that would come out of the paper roll.

It is meant for people who develop point-of-sale (POS) software and need to see, debug
and validate what their system sends to a receipt printer, without a physical printer.

PhantomPOS is an open source project by Tegra Software, built with Rust and Tauri 2.

## Status

Early development. Version 0.1.0 has not been released yet, and no binaries are
published. The project is being built in stages; stages 1 (skeleton) and 2 (text and
formatting) are done.

## Features

What is implemented today:

- A single-window desktop app with light and dark themes.
- A raw TCP listener on `127.0.0.1:9100`.
- An ESC/POS parser for text, styles, alignment, line feeds and code pages (CP437,
  CP850, CP860, CP858 and Windows-1252).
- A log of decoded commands, side by side with the raw bytes, highlighting unknown
  sequences.

Planned for v1 (not implemented yet):

- Configurable port, and an opt-in mode that accepts connections from the local network.
- Parser support for cut, cash drawer, raster images, QR codes and barcodes.
- Real-time rendering of the receipt roll for 58 mm and 80 mm paper.
- Simulated printer states (paper out, cover open, offline, drawer open) with replies
  to the status commands `DLE EOT`, `GS r` and `GS a`.
- Export of a receipt as PNG and of the raw bytes as a `.bin` file.
- Interface in English and Portuguese.

Ideas outside the v1 scope are tracked in [docs/backlog.md](docs/backlog.md).

## Quick try

With the app running, send some bytes to the listener:

```sh
printf 'Hello\n' | nc 127.0.0.1 9100
```

The window shows the received bytes in hexadecimal.

## Downloads

Once released, installers will be published on the
[GitHub Releases](https://github.com/tegrasw/phantompos/releases) page:

| System | Package |
| --- | --- |
| Windows | NSIS installer (`.exe`) |
| macOS | `.dmg`, one per architecture (Apple silicon and Intel) |
| Linux | AppImage and `.deb` |

The Windows installer is not code-signed, so Windows SmartScreen shows a warning the
first time it runs. The Linux `.deb` requires WebKitGTK 4.1.

## Development

### Prerequisites

- Rust, managed by [rustup](https://rustup.rs). The toolchain version is pinned in
  `rust-toolchain.toml` and installed automatically.
- Node.js 24 and pnpm 10.
- The Tauri 2 system dependencies for your platform, listed in the
  [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)
  (on Linux: WebKitGTK 4.1 and related packages).

### Commands

Run the app in development mode:

```sh
cd app
pnpm install
pnpm tauri dev
```

Check, lint and test, as CI does:

```sh
# Frontend (from app/). The build must exist before compiling the Rust app crate.
pnpm check
pnpm build

# Rust (from the repository root)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Build release bundles for the current platform:

```sh
cd app
pnpm tauri build
```

## Repository layout

```
phantompos/
├── core/                 # phantompos-core: ESC/POS parser and receipt model (no I/O)
│   └── tests/fixtures/   # reference captures (.bin) and expected output (.json)
├── app/
│   ├── src-tauri/        # phantompos: Tauri app, TCP listener, state, events
│   └── src/              # user interface: Svelte + TypeScript + Vite
├── docs/                 # backlog and maintainer documentation
└── .github/workflows/    # ci.yml (pull requests, main) and release.yml (tags v*)
```

Releases are built by GitHub Actions when a `v*` tag is pushed; see
[docs/release-signing.md](docs/release-signing.md) for the macOS signing setup.

## License

[MIT](LICENSE). Copyright (c) 2026 Tegra Software.
