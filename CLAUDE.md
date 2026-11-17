# PhantomPOS

Virtual ESC/POS thermal printer: a Tauri 2 desktop app that listens on TCP 9100,
parses ESC/POS and draws the receipt. `SPEC.md` (Portuguese) is the source of
truth; work proceeds one stage at a time ("Etapas e critérios de aceite").
Current stage: **1 (skeleton) done**; stage 2 (text parser) not started.

## Layout

| Path | What |
| --- | --- |
| `core/` | `phantompos-core`: parser + receipt model. No network, disk, Tokio or Tauri. Empty until stage 2. |
| `core/tests/fixtures/` | `.bin` captures + expected `.json` (from stage 2). |
| `app/src-tauri/` | `phantompos`: Tauri shell, TCP listener (`src/listener.rs`), events, commands. |
| `app/src/` | UI: Svelte 5 + TypeScript + Vite. Only `src/lib/backend.ts` talks to Tauri. |
| `.github/workflows/` | `ci.yml` (PRs and main), `release.yml` (tags `v*`). |
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
| Event | `bytes:received` | `{ connectionId: number, peer: string, bytes: number[] }`, one per socket read. **Provisional**: stage 2 replaces it with `commands:received`. |
| Command | `get_state` | `{ listener: <listener:status payload>, connections: <connection:changed payload> }` |

The UI registers its event listeners first and then calls `get_state`.
Still to come from the SPEC: `receipt:completed`, `commands:received`,
`drawer:kicked`, `set_printer_status`, `update_settings`, `export_receipt`.

New commands need a permission: add them to `AppManifest` in
`app/src-tauri/build.rs` and to `app/src-tauri/capabilities/default.json`.

## Conventions

- Code, comments, commit messages and README in English.
- No `unwrap`/`expect` on paths that handle network data. Typed errors in `core`.
- `core` never depends on network, disk, Tokio or Tauri.
- Every new crate or package needs a justification in the commit description.
- Small Conventional Commits, one logical unit each.
- Parser work is test-first: every new command arrives with its fixture.
- ESC/POS doubts: the Epson reference wins; document deliberate deviations in code.
- Signing happens only in `release.yml`, never in CI.

## Known limitations (stage 1)

- Bytes received before the webview has registered its listeners (the first
  moments after launch) are not shown; there is no backlog replay yet.
- Port and bind address are fixed at `127.0.0.1:9100` until stage 6. Flutter
  DevTools also defaults to port 9100; if it is running, the header shows the
  port-in-use error.
