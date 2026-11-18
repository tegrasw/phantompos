#!/usr/bin/env node
// Writes app/src/assets/third-party-licenses.json: every third-party
// component shipped in the PhantomPOS app, with its license texts, for the
// "Third-party licenses" view in Settings > About.
//
//   node scripts/generate-licenses.mjs           regenerate the file
//   node scripts/generate-licenses.mjs --check   exit 1 if the file is stale
//
// Sources:
// - Rust crates: `cargo about generate --format json` (cargo-about 0.9.3,
//   config in /about.toml) for the `phantompos` app crate, over every release
//   target. Workspace crates (ours, MIT) are left out.
// - npm packages bundled into the UI (BUNDLED_NPM below), read from
//   app/node_modules.
// - The embedded font, JetBrains Mono (text from app/src/assets/fonts/OFL.txt).
//
// The output is deterministic (sorted, no timestamps), so `git diff` shows
// exactly what changed. Identical license texts are stored once; components
// point at them by index (`licenseRefs`). See scripts/README.md.

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUTPUT = join(ROOT, "app/src/assets/third-party-licenses.json");
const APP_MANIFEST = join(ROOT, "app/src-tauri/Cargo.toml");
const ABOUT_CONFIG = join(ROOT, "about.toml");
const FONTS = join(ROOT, "app/src/assets/fonts");
const CARGO_ABOUT_VERSION = "0.9.3";

/** Our own crates: MIT, covered by the app's LICENSE. */
const WORKSPACE_CRATES = new Set(["phantompos", "phantompos-core"]);

/**
 * npm packages that end up in the UI bundle. Everything else in
 * package.json is build tooling. To verify after a dependency change, build
 * with `npx vite build --sourcemap --outDir /tmp/x` and list the
 * `node_modules/<package>` entries of the `.map` files' `sources`.
 */
const BUNDLED_NPM = ["@tauri-apps/api", "svelte"];

/** For a choice ("MIT OR Apache-2.0"), the license text we ship, in order (as about.toml). */
const PREFERRED = ["MIT", "Apache-2.0", "BSD-3-Clause", "Zlib", "Unicode-3.0", "MPL-2.0"];

const LICENSE_NAMES = {
  "Apache-2.0": "Apache License 2.0",
  "BSD-3-Clause": 'BSD 3-Clause "New" or "Revised" License',
  MIT: "MIT License",
  "MPL-2.0": "Mozilla Public License 2.0",
  "OFL-1.1": "SIL Open Font License 1.1",
  "Unicode-3.0": "Unicode License v3",
  Zlib: "zlib License",
};

const check = process.argv.includes("--check");

// ---------------------------------------------------------------------------

function run(command, args, options = {}) {
  try {
    return execFileSync(command, args, {
      cwd: ROOT,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      stdio: ["ignore", "pipe", "inherit"],
      ...options,
    });
  } catch (error) {
    if (error.code === "ENOENT") fail(`\`${command}\` was not found on PATH.`);
    fail(`\`${command} ${args.join(" ")}\` failed (exit ${error.status}).`);
  }
}

function fail(message) {
  console.error(`generate-licenses: ${message}`);
  process.exit(1);
}

/** Line endings normalised, trailing whitespace and blank edges removed. */
function normaliseText(text) {
  return text
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => line.trimEnd())
    .join("\n")
    .replace(/^\n+|\n+$/g, "");
}

function normaliseExpression(expression) {
  return expression.replace(/\s+/g, " ").trim();
}

function repositoryUrl(repository) {
  const raw = typeof repository === "string" ? repository : repository?.url;
  if (!raw) return undefined;
  return raw.replace(/^git\+/, "").replace(/\.git$/, "").replace(/^git:\/\//, "https://");
}

// License texts, stored once each ------------------------------------------------

/** key "id\0text" -> { id, name, text } */
const texts = new Map();

function addText(id, text) {
  const normalised = normaliseText(text);
  if (!normalised) fail(`empty license text for ${id}`);
  const key = `${id}\0${normalised}`;
  if (!texts.has(key)) texts.set(key, { id, name: LICENSE_NAMES[id] ?? id, text: normalised });
  return key;
}

const components = [];

// Rust crates ----------------------------------------------------------------------

function crates() {
  const version = run("cargo", ["about", "--version"]).trim();
  if (version !== `cargo-about ${CARGO_ABOUT_VERSION}`) {
    fail(
      `expected cargo-about ${CARGO_ABOUT_VERSION}, found "${version}". Install it with\n` +
        `  cargo install cargo-about --version ${CARGO_ABOUT_VERSION} --locked --features cli`,
    );
  }
  // Download every target's crates first, so cargo-about can run offline:
  // its result then depends only on Cargo.lock and the crate sources.
  run("cargo", ["fetch", "--locked", "--manifest-path", APP_MANIFEST]);
  const json = run("cargo", [
    "about",
    "generate",
    "--format",
    "json",
    "--frozen",
    "--fail",
    "--manifest-path",
    APP_MANIFEST,
    "--config",
    ABOUT_CONFIG,
  ]);
  const about = JSON.parse(json);

  /** crate id -> keys of its license texts */
  const used = new Map();
  for (const license of about.licenses) {
    const key = addText(license.id, license.text);
    for (const { crate } of license.used_by) {
      if (!used.has(crate.id)) used.set(crate.id, new Set());
      used.get(crate.id).add(key);
    }
  }

  for (const { package: pkg, license } of about.crates) {
    if (WORKSPACE_CRATES.has(pkg.name)) continue;
    const keys = used.get(pkg.id);
    if (!keys?.size) fail(`no license text found for crate ${pkg.name} ${pkg.version}`);
    components.push({
      name: pkg.name,
      version: pkg.version,
      kind: "crate",
      license: normaliseExpression(license ?? pkg.license ?? ""),
      keys: [...keys],
      url: repositoryUrl(pkg.repository) ?? pkg.homepage ?? `https://crates.io/crates/${pkg.name}`,
    });
  }
}

// npm packages -----------------------------------------------------------------------

function npmPackages() {
  for (const name of BUNDLED_NPM) {
    const dir = join(ROOT, "app/node_modules", name);
    let pkg;
    try {
      pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    } catch {
      fail(`${name} is not installed; run \`pnpm install\` in app/ first.`);
    }
    const expression = normaliseExpression(pkg.license ?? "");
    // The license we ship from a choice, and the file holding its text.
    const ids = expression.split(/ OR /).map((part) => part.replace(/[()]/g, "").trim());
    const id = PREFERRED.find((candidate) => ids.includes(candidate));
    if (!id || / AND /.test(expression)) fail(`unsupported license "${expression}" for ${name}`);
    const files = readdirSync(dir).sort();
    const file =
      files.find((f) => f.toUpperCase() === `LICENSE-${id.toUpperCase()}`) ??
      (ids.length === 1 ? files.find((f) => /^(LICEN[CS]E|COPYING)(\.(md|txt))?$/i.test(f)) : undefined);
    if (!file) fail(`no ${id} license file in ${dir}`);
    components.push({
      name,
      version: pkg.version,
      kind: "npm",
      license: expression,
      keys: [addText(id, readFileSync(join(dir, file), "utf8"))],
      url: pkg.homepage?.replace(/#readme$/, "") ?? repositoryUrl(pkg.repository),
    });
  }
}

// Font --------------------------------------------------------------------------------

function font() {
  const readme = readFileSync(join(FONTS, "README.md"), "utf8");
  const version = /JetBrains Mono (\d+(?:\.\d+)+) release/.exec(readme)?.[1];
  if (!version) fail("could not read the JetBrains Mono version from app/src/assets/fonts/README.md");
  components.push({
    name: "JetBrains Mono",
    version,
    kind: "font",
    license: "OFL-1.1",
    keys: [addText("OFL-1.1", readFileSync(join(FONTS, "OFL.txt"), "utf8"))],
    url: "https://github.com/JetBrains/JetBrainsMono",
  });
}

// Output --------------------------------------------------------------------------------

function compare(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

function build() {
  crates();
  npmPackages();
  font();

  const licenses = [...texts.entries()]
    .sort(([, a], [, b]) => compare(a.id, b.id) || compare(a.text, b.text))
    .map(([key, license], index) => ({ key, index, license }));
  const indexOf = new Map(licenses.map(({ key, index }) => [key, index]));

  const KIND_ORDER = { font: 0, npm: 1, crate: 2 };
  components.sort(
    (a, b) =>
      compare(a.name.toLowerCase(), b.name.toLowerCase()) ||
      compare(a.name, b.name) ||
      compare(a.version, b.version) ||
      KIND_ORDER[a.kind] - KIND_ORDER[b.kind],
  );

  // One license or component per line: compact, and diffs stay readable.
  const lines = [
    "{",
    `  "generator": "scripts/generate-licenses.mjs (do not edit by hand)",`,
    `  "licenses": [`,
    licenses.map(({ license }) => `    ${JSON.stringify(license)}`).join(",\n"),
    "  ],",
    `  "components": [`,
    components
      .map(({ keys, ...component }) =>
        `    ${JSON.stringify({
          name: component.name,
          version: component.version,
          kind: component.kind,
          license: component.license,
          licenseRefs: keys.map((key) => indexOf.get(key)).sort((a, b) => a - b),
          ...(component.url ? { url: component.url } : {}),
        })}`,
      )
      .join(",\n"),
    "  ]",
    "}",
    "",
  ];
  return lines.join("\n");
}

const output = build();

if (check) {
  let current = "";
  try {
    current = readFileSync(OUTPUT, "utf8");
  } catch {
    // Missing counts as stale.
  }
  if (current !== output) {
    console.error(
      "generate-licenses: app/src/assets/third-party-licenses.json is stale.\n" +
        "Run `node scripts/generate-licenses.mjs` and commit the result.",
    );
    process.exit(1);
  }
  console.log("third-party-licenses.json is up to date.");
} else {
  writeFileSync(OUTPUT, output);
  const counts = {};
  for (const { kind } of components) counts[kind] = (counts[kind] ?? 0) + 1;
  console.log(
    `Wrote app/src/assets/third-party-licenses.json: ${components.length} components ` +
      `(${Object.entries(counts)
        .map(([kind, n]) => `${n} ${kind}`)
        .join(", ")}), ${texts.size} license texts, ${(Buffer.byteLength(output) / 1024).toFixed(1)} KiB.`,
  );
}
