<img src="app/src-tauri/icons/128x128@2x.png" alt="PhantomPOS icon" width="96" height="96">

# PhantomPOS

PhantomPOS is a virtual ESC/POS thermal printer. It is a desktop application that
listens on TCP port 9100, interprets ESC/POS commands and shows on screen the receipt
that would come out of the paper roll.

It is meant for people who develop point-of-sale (POS) software and need to see, debug
and validate what their system sends to a receipt printer, without a physical printer.

PhantomPOS is an open source project by Tegra Software, built with Rust and Tauri 2.

## Status

Feature-complete for version 0.1.0: everything planned for the first version is
implemented and covered by tests. The 0.1.0 release itself is being prepared, so no
installers are published yet; until then, build from source as described under
[Development](#development).

## Features

- A single-window desktop app with light and dark themes.
- A raw TCP listener on `127.0.0.1:9100`, like a networked receipt printer.
- An ESC/POS parser for text, styles, alignment, line feeds and code pages (CP437,
  CP850, CP860, CP858 and Windows-1252).
- Cut and cash drawer commands.
- Raster images (`GS v 0`), QR codes and barcodes (UPC-A, UPC-E, EAN-13, EAN-8,
  CODE39, ITF, CODABAR, CODE93 and CODE128).
- Real-time rendering of the receipt roll for 58 mm and 80 mm paper, with the cut
  visible between receipts and a cash drawer indicator.
- Simulated printer states (paper out, cover open, offline, drawer open) with replies
  to the status commands `DLE EOT`, `GS r` and `GS a`.
- Settings saved between runs: port, local-only or local-network access (with a
  warning, since raw printing has no authentication), paper width and default code page.
  If the port is in use, the app offers to use the next free one; it never switches
  ports on its own.
- Export of a receipt as PNG and of its raw bytes as a `.bin` file, which works as a
  test fixture in `core/tests/fixtures/`.
- Interface in English and Brazilian Portuguese, following the system language.
- A log of decoded commands, side by side with the raw bytes, highlighting unknown
  sequences.

## Usage

Point your POS software at PhantomPOS as if it were a network receipt printer: raw TCP
(often called "port 9100" or JetDirect) at the address shown in the app header, by
default `127.0.0.1:9100`. To print from another device, enable local-network access in
the settings and use one of the addresses the header then lists.

With the app running, you can also send a test print from a terminal:

```sh
printf '\033@\033a\001\033E\001PhantomPOS\033E\000\nHello, receipt!\n\n\n\035V\000' | nc 127.0.0.1 9100
```

The receipt appears on the roll, and the command log shows each decoded command next to
its raw bytes. The repository also has ready-made samples:

```sh
cat core/tests/fixtures/receipt_codes.bin | nc 127.0.0.1 9100
```

If another program already uses port 9100 (Flutter DevTools does by default), the app
says so and offers to use the next free port.

## Scope and limitations

PhantomPOS follows the Epson ESC/POS standard and aims to match columns, line breaks,
alignment and sizes; it does not try to reproduce a real printer dot for dot.

- Only raw TCP is supported: no USB, serial or Bluetooth, and it does not install as a
  system printer.
- Images are drawn for `GS v 0` only. Other graphics commands (`ESC *`, `GS ( L`,
  `GS 8 L`) and 2D symbols other than QR codes (such as PDF417) are recognized and
  logged, but not drawn.
- Vendor-specific commands (Bematech, Elgin, Daruma) are not interpreted.
- Receipts are kept only while the app is open.

Ideas outside the v1 scope are tracked in [docs/backlog.md](docs/backlog.md).

## Downloads

Once version 0.1.0 is released, installers will be published on the
[GitHub Releases](https://github.com/tegrasw/phantompos/releases) page:

| System | Package |
| --- | --- |
| Windows | NSIS installer (`.exe`) |
| macOS | `.dmg`, one per architecture (Apple silicon and Intel), signed and notarized |
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

Regenerate the third-party licenses shown in the app after a dependency
change (needs `cargo-about` 0.9.3; see [scripts/README.md](scripts/README.md)):

```sh
node scripts/generate-licenses.mjs          # rewrite app/src/assets/third-party-licenses.json
node scripts/generate-licenses.mjs --check  # fail if it is stale
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
├── scripts/              # maintenance scripts (third-party licenses)
└── .github/workflows/    # ci.yml (pull requests, main) and release.yml (tags v*)
```

Releases are built by GitHub Actions when a `v*` tag is pushed; see
[docs/release-signing.md](docs/release-signing.md) for the macOS signing setup.

## Credits

PhantomPOS is developed by [Tegra Software](https://tegra.app); author: Renan
Galeno.

Built with [Tauri](https://tauri.app), [Svelte](https://svelte.dev) and the
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) typeface (SIL Open
Font License 1.1). The licenses of every third-party component shipped in the
app are listed in the app under Settings > About > Third-party licenses, and in
[app/src/assets/third-party-licenses.json](app/src/assets/third-party-licenses.json).

ESC/POS is a trademark of Seiko Epson Corporation. PhantomPOS is not
affiliated with Epson.

## License

[MIT](LICENSE). Copyright (c) 2026 Tegra Software Ltda.
