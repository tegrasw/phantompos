# PhantomPOS

Virtual ESC/POS thermal printer: a Tauri 2 desktop app that listens on TCP 9100,
parses ESC/POS and draws the receipt. `SPEC.md` (Portuguese) is the source of
truth; work proceeds one stage at a time ("Etapas e critérios de aceite").
Current stage: **6 (settings and export) done**. All six stages are implemented; macOS signing and notarization are set up and verified (tag `v0.1.0-test.3`). What remains before 0.1.0: a test with real POS receipts.

## Layout

| Path | What |
| --- | --- |
| `core/` | `phantompos-core`: parser + receipt model. No network, disk, Tokio or Tauri. API and JSON shapes in `core/README.md`. |
| `core/tests/fixtures/` | `.bin` captures + expected `.json`; `generate.py` (python-escpos) and a README listing every fixture. |
| `app/src-tauri/` | `phantompos`: Tauri shell, TCP listener (`src/listener.rs`), events, commands. |
| `app/src/` | UI: Svelte 5 + TypeScript + Vite. Only `src/lib/backend.ts` talks to Tauri. |
| `.github/workflows/` | `ci.yml` (PRs and main; skipped for documentation-only changes), `release.yml` (tags `v*`). |
| `design/icon/` | Logo and icon sources: `AppIcon.icon` (Icon Composer, Liquid Glass), flat masters, `mark.svg`, `build.sh`. |
| `scripts/`, `about.toml` | `generate-licenses.mjs` builds `app/src/assets/third-party-licenses.json` with cargo-about. |
| `docs/backlog.md` | Out-of-scope ideas. Never implement them. |
| `docs/release-signing.md` | Apple secrets needed by `release.yml` and how to obtain them. |

Cargo workspace at the repo root (`Cargo.lock` and `target/` live there). The
frontend package and the Tauri CLI live in `app/`.

## Commands

```bash
# Run the app (from app/)
pnpm install
pnpm tauri dev

# Frontend checks (from app/)
pnpm check        # svelte-check + tsc
pnpm build        # Vite build to app/dist

# Rust checks (from the repo root) - same as CI
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Local release bundle (from app/)
pnpm tauri build

# Smoke test while the app is running
printf 'Hello\n' | nc 127.0.0.1 9100
cat core/tests/fixtures/receipt_small.bin | nc 127.0.0.1 9100

# Rewrite fixture JSON after an intended parser change (review the diff)
UPDATE_FIXTURES=1 cargo test -p phantompos-core --test fixtures

# Regenerate the third-party license list after changing Cargo.lock, the
# bundled npm packages or the font (--check verifies it is current)
node scripts/generate-licenses.mjs

# UI with mock data in a plain browser (dev only, from app/)
pnpm dev   # then open http://localhost:1420/?demo (also ?demo=gallery, cap, status, portinuse, network, noaddr, tall)
```

`app/dist` must exist before a release compile of the app crate
(`tauri::generate_context!` embeds it); debug builds use the dev server.

## Versions (chosen 2026-11-17, current stable at the time)

| Component | Version |
| --- | --- |
| Rust toolchain | 1.100.0 (pinned in `rust-toolchain.toml`), edition 2024 |
| tauri / tauri-build | 2.12.6 / 2.7.5 |
| tokio | 1.54.1 |
| serde / thiserror | 1.0.229 / 2.0.21 |
| tauri-plugin-dialog (Rust only, no JS package) | 2.8.4 |
| if-addrs | 0.15.0 |
| tauri-plugin-opener (Rust only) | 2.7.3 |
| cargo-about (dev tool, install with `--features cli`) | 0.9.3 |
| qrcode (core, default features off) | 0.14.1 |
| @tauri-apps/api, @tauri-apps/cli | 2.12.5 |
| Svelte | 5.57.3 |
| Vite / @sveltejs/vite-plugin-svelte | 8.3.8 / 7.3.1 |
| TypeScript | 6.0.3 (svelte-check 4.7.8 does not accept TypeScript 7 yet) |
| Node / pnpm | 24 / 10 |
| tauri-action | `tauri-apps/tauri-action@v1` (release `action-v1.0.0`) |

Before using any Tauri, Tokio, Svelte or tauri-action API, read the current
docs; do not rely on memory. Update this table when a version changes.

## Backend / UI contract

All payloads are camelCase JSON. Names are declared once on each side:
`app/src-tauri/src/lib.rs` (`events` module) and `app/src/lib/backend.ts`.

| Kind | Name | Payload |
| --- | --- | --- |
| Event | `listener:status` | `{ state: "listening" \| "error" \| "stopped", address, port, addresses: string[], message?, code?: "portInUse" \| "permissionDenied" \| "other", suggestedPort? }`. `address` is the bind address; `addresses` is what to show (`127.0.0.1` in local mode, every non-loopback, non-link-local IPv4 in network mode, possibly empty). `suggestedPort` only on `portInUse`. |
| Event | `settings:changed` | Full `Settings = { port, networkMode: "local" \| "network", paper: "58mm" \| "80mm", defaultCodePage: "cp437" \| "cp850" \| "cp860" \| "cp858" \| "windows1252" }`. |
| Event | `connection:changed` | `{ active: number, peers: string[] }` |
| Event | `commands:received` | `{ connectionId: number, peer: string, commands: DecodedCommand[] }`, never empty. `DecodedCommand = { offset, bytes: number[], command: { type, ...params } }`; `offset` counts from the start of the connection stream. Variants and fields: `core/README.md`. Pending text and incomplete sequences are flushed on disconnect, before `connection:changed`. |
| Event | `receipt:updated` | `{ connectionId, peer, receipt: Receipt }`: full snapshot of that connection's in-progress receipt (`cut: null`). Only when something new was printed; throttled per connection to one per 50 ms (the first change is sent at once, a pending snapshot is dropped when its receipt completes). |
| Event | `receipt:completed` | `{ id, connectionId, peer, receipt: Receipt }`: closed by a cut (`cut: "full" \| "partial"`) or by the end of the connection (`cut: null`). `id` is unique and increasing in closing order. |
| Event | `drawer:kicked` | `{ connectionId, peer, pin: 2 \| 5, onMs, offMs }`. The drawer stays open until the UI closes it with `set_printer_status`. |
| Event | `printer:status` | Full `PrinterStatus = { paperOut, coverOpen, offline, drawerOpen }`, only when it changes (UI command or drawer kick). |
| Event | `status:sent` | `{ connectionId, peer, request, reply: { kind: "realTime" \| "transmit" \| "automatic", bytes, flags } }` for every reply written to a socket. `request` is a mnemonic such as `DLE EOT 1`, `GS r 49` or `ASB`; `flags` are decoded from the bytes (vocabulary in `core/README.md`). |
| Event | `print:suppressed` | `{ connectionId, peer, reason: "coverOpen" \| "paperOut" \| "offline", bytes }`, at most one per socket read while an error state is active (reason priority in that order). |
| Command | `get_state` | `{ listener, connections, settings: Settings, status: PrinterStatus }` |
| Command | `update_settings` | Argument `settings`: a partial `Settings` (invalid values reject). Persists, emits `settings:changed`, and restarts only the listener when `port` or `networkMode` changed. `paper` applies to the next receipt, `defaultCodePage` to the next connection. Returns the full settings. |
| Command | `retry_listener` | No arguments. Binds again with the current settings; the result arrives as `listener:status`. |
| Command | `export_receipt` | Arguments `id` (from `receipt:completed`), `format: "png" \| "bin"`, `png?: number[]` (PNG file bytes from the UI renderer). The backend opens the native save dialog and writes the file. Returns `{ saved: true, path }` or `{ saved: false }` on cancel. |
| Command | `open_link` | Argument `link: "website" \| "repository" \| "issues" \| "license"`: an id, never a URL. Rust maps it to a fixed address (`src/links.rs`) and opens the system browser; the webview has no opener permission. |
| Command | `clear_receipts` | No arguments. Frees the retained raw bytes; the UI calls it on "Clear roll". |
| Command | `set_printer_status` | Argument `status`: a partial `PrinterStatus`, merged into the current one (unknown fields reject). Returns the full status. |

Order per socket read: `commands:received`, `print:suppressed`, then
`receipt:completed` / `drawer:kicked` / `printer:status` in stream order,
`status:sent` for each reply written, then `receipt:updated`. On disconnect: flushed
commands, `receipt:completed` if content is pending, then `connection:changed`.
`Receipt` is documented in `core/README.md` (all values in printer dots). Its
elements are `line` (text spans) and `image` (`kind`: `raster`, `qrCode` or
`barcode`; 1-bit rows as base64). QR codes and barcodes are generated in core
(owner decision, deviating from the SPEC's "JavaScript libraries") and reach
the UI as bitmaps; barcode HRI text is ordinary `line` elements.

The UI registers its event listeners first and then calls `get_state`.
Settings live in `settings.json` in the app config directory (macOS:
`~/Library/Application Support/app.tegra.oss.phantompos/`), written atomically
and loaded leniently (a bad field falls back to its default). A listener
restart closes the old connections like normal disconnects, then reports
`stopped`, then `listening` or `error`. Raw bytes are kept for the last 200
completed receipts (32 MiB total). A `.bin` holds the receipt's own bytes: from
the end of the previous receipt (or the connection start) through its closing
cut; `SessionEvent::ReceiptCompleted` carries that end offset.

Status replies are computed in core by pure functions of `PrinterStatus`
(`core/src/status.rs`); the reply table per command and state is in
`core/README.md` and asserted by `core/tests/status_table.rs`. Automatic Status
Back is enabled per connection by `GS a`; the status itself is global. While
paper out, cover open or offline is active, everything except status commands
is discarded for printing. Replies are written with a 2 s timeout; a client
that never reads them is disconnected. A fixture can declare its state in the
file name (`<name>.<states joined by +>.bin`).

New commands need a permission: add them to `AppManifest` in
`app/src-tauri/build.rs` and to `app/src-tauri/capabilities/default.json`.

## ESC/POS reference

The Epson reference URL in SPEC.md is retired. The live one is
https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/ (it rejects curl
and WebFetch with 403; read it in a real browser).

## UI and visual quality

The owner requires a premium-looking app: every stage that touches the UI
includes visual design work, not just function.

- Design tokens (colours for both themes, type scale, spacing, motion) live at
  the top of `app/src/app.css`; `app/src/lib/motion.ts` mirrors the durations.
- One accent colour (teal). Red only for listener errors, amber only for
  unknown sequences, and the calm indigo `--fault` tokens only for simulated
  printer faults (they are states the user switched on, not app errors). Text contrast at least WCAG AA in both themes.
- All motion respects `prefers-reduced-motion`.
- Embedded font: JetBrains Mono (OFL) in `app/src/assets/fonts/`. The UI uses
  the latin and latin-ext subsets; the receipt canvas uses a separate family,
  "PhantomPOS Receipt", subset to every character the five code pages can
  produce (recipe in the fonts README).
- Receipt rendering (`app/src/lib/receipt/`): the bitmap is exactly `width`
  dots wide, one pixel per dot, thresholded to pure black and white, and
  scaled without smoothing. Each character is drawn in its own cell; box
  drawing and block characters are drawn procedurally, not from the font.
  Receipts are split into 1024-dot tiles that release their bitmap when far
  off-screen. One zoom for all receipts keeps 58 and 80 mm in proportion.
  `renderReceipt()` with `EXPORT_COLORS` gives black on white for PNG export.
- Image elements are written dot for dot after the text threshold, cached by
  element and by data string. The zoom prefers a whole number of device pixels
  per dot and sheets sit on whole device pixels, so codes stay scannable;
  clicking a QR code or barcode opens a close-up (`CodePreview`).
- The command log keeps only the first 256 bytes of each command (plus its real
  length); raw-bytes export in stage 6 must come from the backend.
- Layout: state panel on the left (272 px, collapses to a rail), command log on
  the right, collapsed by default (owner decision). Panels are opened from
  their rails and closed from their own headers; the app header has no panel
  toggles. Both can be open only from 912 px of window width; below that,
  opening one collapses the other.
- Interface in English and Brazilian Portuguese (`app/src/lib/i18n/`): `en.ts`
  defines the shape and `pt.ts` must match it (type-checked). The first `pt*`
  or `en*` entry of `navigator.languages` wins; `?lang=pt|en` in dev. ESC/POS
  mnemonics, code page names and raw enum values stay untranslated; unknown
  command types and flags fall back to a humanized English identifier.
- Settings open in a modal from the header gear; enabling network access needs
  an explicit confirmation inside it. PNG export is a 1-bit PNG built in bands
  with `CompressionStream` (no height limit); the canvas fallback caps at
  16,384 rows. `__APP_VERSION__` comes from `app/package.json`: keep it in sync
  with the Cargo version.
- The roll keeps at most 200 entries (receipts and "not printed" notices;
  consecutive notices of the same connection and reason coalesce).
- No component or animation libraries; inline SVG icons.
- The UI renders commands generically from `command.type` and its params, so
  new core variants need no UI change.
- Verify visually (both themes, 1200x800 and 720x480) and attach screenshots to
  the stage report. Screenshots of the real window are blank while the Mac
  screen is locked.

## Logo and icons

The mark is a ghost made of receipt paper coming out of a printer slot. Sources
live in `design/icon/`; everything in `app/src-tauri/icons/` is generated.

| Source | Becomes |
| --- | --- |
| `AppIcon.icon` (layers in `Assets/*.svg`, look in `icon.json`) | `icons/Assets.car`: the macOS 26+ Liquid Glass icon (asset `AppIcon`) |
| `icon-macos.svg` (tile on Apple's 824/1024 grid) | `icons/icon.icns`: fallback for macOS before 26 |
| `icon-full.svg` (near full-bleed tile) | `icons/icon.ico` and the PNGs (Windows, Linux) |
| `mark.svg` (24x24, `currentColor`) | copied by hand into `app/src/lib/components/AppMark.svelte` |

```bash
design/icon/build.sh            # regenerate app/src-tauri/icons/ (macOS, Xcode 26+)
design/icon/build.sh previews   # render all six appearances to design/icon/preview/ (ignored by git)
open design/icon/AppIcon.icon   # edit in Icon Composer
```

- The layer shapes are repeated in `icon-macos.svg`, `icon-full.svg` and
  `mark.svg`; change them together.
- `bundle.icon` lists the precompiled `Assets.car`, not the `.icon`: the
  bundler's own actool call fails on Xcode 27 and some 26.x
  (tauri-apps/tauri#16386, #15315). The bundler copies the `.car` to
  `Contents/Resources` and sets `CFBundleIconName` before signing. Once that is
  fixed upstream the `.icon` can be listed directly.
- If actool reports `Bad file descriptor`, a failed bundler run left a broken
  `ibtoold` daemon behind: `killall ibtoold` and run `build.sh` again.
- `Assets.car` and `icon.icns` are not byte-reproducible; commit them only
  when the artwork changed.
- `icon.json` has no public schema; edit it in Icon Composer or check the
  result with `build.sh previews`.
- `tauri icon` knows nothing about `.icon`; `build.sh` runs it twice (one
  master per platform family) and drops the iOS/Android output.

## Owner decisions for later stages

- Port in use: the error state offers actions ("Use <next free port>" and
  "Try again"). The app never switches ports on its own: no silent automatic
  fallback (decided 2026-11-18, implemented in stage 6).

## Conventions

- Code, comments, commit messages and README in English.
- No `unwrap`/`expect` on paths that handle network data. Typed errors in `core`.
- `core` never depends on network, disk, Tokio or Tauri.
- Every new crate or package needs a justification in the commit description,
  and may need its license added to `accepted` in `about.toml`; regenerate the
  third-party list afterwards (the About screen shows it).
- Neither crate is published to crates.io (`publish = false` in both, owner
  decision); distribution is the installers on GitHub Releases.
- Copyright holder: "Tegra Software Ltda" (legal name). Credits: Tegra
  Software, author Renan Galeno, https://tegra.app.
- Small Conventional Commits, one logical unit each.
- Parser work is test-first: every new command arrives with its fixture.
- ESC/POS doubts: the Epson reference wins; document deliberate deviations in code.
- Signing happens only in `release.yml`, never in CI.

## Known limitations (stage 6)

- Commands received before the webview has registered its listeners (the
  first moments after launch) are not shown; there is no backlog replay yet.
- Text is emitted when its run ends (next command, 4096 bytes, or
  disconnect), so a client that keeps the socket open sees trailing text late.
- Status bytes inside another command's data (an image, QR or barcode payload)
  are not executed; a real Epson would execute them. `GS r` and `GS a` are
  answered at once even in an error state.
- Data received in an error state is discarded, not printed after recovery.
- Not simulated: near-end sensor, feed button, printer errors, ink, peeler and
  power-off replies. `GS ( k` fn 82 (QR size request) gets no reply.
- The state panel's "reports" summary is derived from the switches, not from
  actual replies.
- Other graphics (`ESC *`, `GS ( L`, `GS 8 L`, `GS *`, `GS /`, `FS p`) and 2D
  symbols other than QR are consumed by length and logged as unsupported, not
  drawn. GS1 barcodes and CODE128 auto are logged as unsupported.
- Image or symbol payloads over 1 MiB are consumed but not stored.
- An image, barcode or QR print that arrives with text pending on the line is
  ignored, like a cut. Codes wider than the paper are not printed; raster
  images are clipped. No quiet zones are added.
- QR model 1 is drawn as model 2; `GS !` does not scale QR codes.
- `commands:received` carries image bytes as a JSON number array (about 260 KB
  for a 576x1000 raster).
- A cut in the middle of a line (buffered text without LF) is ignored, as on
  an Epson printer. `GS V` function C is drawn like function B.
- Text without its LF is not on the in-progress receipt until an LF, a cut or
  the disconnect.
- Vertical box-drawing lines only connect when line spacing is at most the
  cell height (`ESC 3 24`), as on a real printer.
- Flutter DevTools also defaults to port 9100; if it is running, the app shows
  the port-in-use error with its actions. On macOS, network mode may still bind
  `0.0.0.0:9100` while another program holds `127.0.0.1:9100` (`SO_REUSEADDR`):
  local connections then reach the other program.
- A `.bin` export does not include state inherited from an earlier receipt on
  the same connection (styles, code page), so it may not replay identically.
- The suggested port may be taken again before the user accepts it.
