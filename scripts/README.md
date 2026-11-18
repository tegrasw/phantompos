# Scripts

## `generate-licenses.mjs`

Writes `app/src/assets/third-party-licenses.json`, the list shown in
Settings > About > Third-party licenses: every third-party component shipped
in the app, with its license texts.

```bash
# One-time setup (from anywhere)
cargo install cargo-about --version 0.9.3 --locked --features cli

# Regenerate (from the repo root; needs app/node_modules, so `pnpm install` in app/ first)
node scripts/generate-licenses.mjs

# Fail (exit 1) if the committed file is stale
node scripts/generate-licenses.mjs --check
```

Run it after any change to `Cargo.lock`, to the npm packages bundled into the
UI, or to the embedded font, and commit the JSON with that change.

What it collects:

| Kind | Source |
| --- | --- |
| `crate` | `cargo about generate --format json --frozen --fail` for `app/src-tauri/Cargo.toml`, configured by `/about.toml`: the targets of every release installer (macOS arm64 and x86_64, Windows x86_64, Linux x86_64), build and dev dependencies excluded. Our workspace crates are left out. |
| `npm` | The packages bundled into the UI (`BUNDLED_NPM` in the script: `svelte`, `@tauri-apps/api`), from `app/node_modules`. |
| `font` | JetBrains Mono: version from `app/src/assets/fonts/README.md`, text from `OFL.txt`. |

For a choice such as `MIT OR Apache-2.0`, the shipped text is the first
license of `accepted` in `about.toml` (MIT first). A new dependency whose
license is not in `accepted` makes the script fail: review the license, then
add it to `about.toml` (and to `PREFERRED`/`LICENSE_NAMES` in the script).

The output is deterministic: crates are downloaded first (`cargo fetch`) and
cargo-about then runs offline, entries are sorted, and there is no
timestamp. Each license text appears once in `licenses`; each component lists
the indices of its texts in `licenseRefs`.

Suggested CI step (not wired in): `node scripts/generate-licenses.mjs --check`
after `pnpm install`, with cargo-about 0.9.3 installed.
