# Embedded fonts

All files are JetBrains Mono (SIL Open Font License 1.1, see `OFL.txt`; the
licence declares no Reserved Font Name).

| Files | Used by | Source |
| --- | --- | --- |
| `jetbrains-mono-latin*-{400,700}-normal.woff2` | UI chrome (`--font-mono`) | `@fontsource/jetbrains-mono` 5.3.0, latin and latin-ext subsets |
| `jetbrains-mono-receipt-{400,700}.woff2` | Receipt canvas (`src/lib/receipt/font.ts`) | JetBrains Mono 2.304 release, subset as below |

## Receipt subset

The receipt font covers every character the printer's code pages can produce:
bytes 0x20-0xFF of CP437, CP850, CP858, CP860 and Windows-1252 (294 code
points, including box drawing, block and shade characters, Greek letters and
math symbols), plus U+FFFD. Layout features are dropped, so programming
ligatures can never merge receipt characters.

JetBrains Mono 2.304 has no glyph for six of those characters: U+2017 `‗`,
U+207F `ⁿ`, U+20A7 `₧`, U+2310 `⌐`, U+2320 `⌠`, U+2321 `⌡`. The renderer draws
them with the system monospace font, squeezed into the cell. Box drawing
(U+2500-257F) and block/shade characters (U+2580-25A0) are drawn as dot
patterns by the renderer, not from the font, so lines always connect.

How the files were produced (fonttools 4.x, in a virtualenv outside the repo):

```bash
# JetBrainsMono-2.304.zip from https://github.com/JetBrains/JetBrainsMono/releases
python3 - > unicodes.txt <<'EOF'
chars = {" "}
for page in ["cp437", "cp850", "cp858", "cp860", "cp1252"]:
    for b in range(0x20, 0x100):
        if b == 0x7F:
            continue
        try:
            chars.add(bytes([b]).decode(page))
        except UnicodeDecodeError:
            pass
print(",".join(f"U+{ord(c):04X}" for c in sorted(chars)))
EOF
for w in Regular:400 Bold:700; do
  pyftsubset "fonts/ttf/JetBrainsMono-${w%%:*}.ttf" \
    --unicodes-file=unicodes.txt --unicodes=U+FFFD \
    --layout-features='' --name-IDs='*' --notdef-outline --flavor=woff2 \
    --output-file="jetbrains-mono-receipt-${w##*:}.woff2"
done
```
