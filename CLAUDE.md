# PhantomPOS

Virtual ESC/POS thermal printer: a Tauri 2 desktop app that listens on TCP 9100,
parses ESC/POS and draws the receipt. `SPEC.md` (Portuguese) is the source of
truth; work proceeds one stage at a time ("Etapas e critérios de aceite").
Current stage: **2 (text and formatting) done**; stage 3 (roll, cut, drawer) not started.

## Layout

| Path | What |
| --- | --- |
| `core/` | `phantompos-core`: parser + receipt model. No network, disk, Tokio or Tauri. API and JSON shapes in `core/README.md`. |
| `core/tests/fixtures/` | `.bin` captures + expected `.json`; `generate.py` (python-escpos) and a README listing every fixture. |
| `app/src-tauri/` | `phantompos`: Tauri shell, TCP listener (`src/listener.rs`), events, commands. |
| `app/src/` | UI: Svelte 5 + TypeScript + Vite. Only `src/lib/backend.ts` talks to Tauri. |
| `.github/workflows/` | `ci.yml` (PRs and main; skipped for documentation-only changes), `release.yml` (tags `v*`). |
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

# UI with mock data in a plain browser (dev only, from app/)
pnpm dev   # then open http://localhost:1420/?demo
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
| Event | `listener:status` | `{ state: "listening" \| "error" \| "stopped", address, port, message? }` (`message` only on error) |
| Event | `connection:changed` | `{ active: number, peers: string[] }` |
| Event | `commands:received` | `{ connectionId: number, peer: string, commands: DecodedCommand[] }`, never empty. `DecodedCommand = { offset, bytes: number[], command: { type, ...params } }`; `offset` counts from the start of the connection stream. Variants and fields: `core/README.md`. Pending text and incomplete sequences are flushed on disconnect, before `connection:changed`. |
| Command | `get_state` | `{ listener: <listener:status payload>, connections: <connection:changed payload> }` |

The UI registers its event listeners first and then calls `get_state`.
Still to come from the SPEC: `receipt:completed`,
`drawer:kicked`, `set_printer_status`, `update_settings`, `export_receipt`.

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
  unknown sequences. Text contrast at least WCAG AA in both themes.
- All motion respects `prefers-reduced-motion`.
- Embedded font: JetBrains Mono (OFL) in `app/src/assets/fonts/`, latin and
  latin-ext only. The stage 3 roll needs box-drawing and shade glyphs
  (U+2500-259F) for CP437, which these subsets lack.
- No component or animation libraries; inline SVG icons.
- The UI renders commands generically from `command.type` and its params, so
  new core variants need no UI change.
- Verify visually (both themes, 1200x800 and 720x480) and attach screenshots to
  the stage report. Screenshots of the real window are blank while the Mac
  screen is locked.

## Conventions

- Code, comments, commit messages and README in English.
- No `unwrap`/`expect` on paths that handle network data. Typed errors in `core`.
- `core` never depends on network, disk, Tokio or Tauri.
- Every new crate or package needs a justification in the commit description.
- Small Conventional Commits, one logical unit each.
- Parser work is test-first: every new command arrives with its fixture.
- ESC/POS doubts: the Epson reference wins; document deliberate deviations in code.
- Signing happens only in `release.yml`, never in CI.

## Known limitations (stage 2)

- Commands received before the webview has registered its listeners (the
  first moments after launch) are not shown; there is no backlog replay yet.
- Text is emitted when its run ends (next command, 4096 bytes, or
  disconnect), so a client that keeps the socket open sees trailing text late.
- Later-stage commands (cut, drawer, raster, QR, barcode, status) appear as
  `unknown`, and their parameter bytes as text or control characters.
- The receipt model is built in core but not yet sent to the UI (stage 3).
- Port and bind address are fixed at `127.0.0.1:9100` until stage 6. Flutter
  DevTools also defaults to port 9100; if it is running, the header shows the
  port-in-use error.
