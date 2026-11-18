# phantompos-core

ESC/POS parser and receipt model for PhantomPOS. No network, disk, Tokio or
Tauri: bytes in, typed commands and a receipt model out, both serializable to
JSON with `serde`.

## Usage (one parser and one builder per connection)

```rust
use phantompos_core::{Effect, Parser, ParserConfig, ReceiptBuilder, ReceiptConfig};

let mut parser = Parser::new(ParserConfig::default()); // default code page CP437
let mut builder = ReceiptBuilder::new(ReceiptConfig::default()); // 80 mm, CR ignored

// Every socket read:
let batch = parser.feed(&chunk);            // Vec<DecodedCommand>, may be empty
builder.set_paper(current_paper);           // optional; see below
for effect in builder.apply_all(&batch) {   // the same batch goes to the command log
    match effect {
        Effect::ReceiptCompleted(receipt) => { /* closed by a cut; receipt.cut is Some */ }
        Effect::DrawerKick(kick) => { /* kick.pin (2 | 5), kick.on_ms, kick.off_ms */ }
    }
}
if let Some(in_progress) = builder.take_update() { /* changed and non-empty; cut None */ }

// Connection closed:
let tail = parser.finish();                 // pending text / incomplete command
let effects = builder.apply_all(&tail);
let last = builder.finish();                // Option<Receipt>, cut None; None if empty
```

`ReceiptBuilder` API:

| Method | What |
| --- | --- |
| `apply(&Command) -> Option<Effect>` | One command; a cut or drawer kick may produce an effect. |
| `apply_all(batch) -> Vec<Effect>` | Every command of a batch, effects in stream order. |
| `take_update() -> Option<Receipt>` | The in-progress receipt if it changed since the last call and has something on paper. |
| `snapshot() -> Option<Receipt>` | The in-progress receipt (`cut: null`), `None` if nothing is on paper. |
| `finish() -> Option<Receipt>` | End of stream: prints the pending line and closes the receipt with `cut: null`. The builder stays usable. |
| `set_paper(PaperProfile)` / `paper()` | Paper for receipts started from now on; a receipt with content keeps its paper and the change applies after it closes. |
| `is_empty()` | Nothing printed and nothing in the print buffer. |

`Effect` is `ReceiptCompleted(Receipt)` or `DrawerKick(DrawerKick)`; the
backend never needs to look at commands. `parse(bytes, config)` and
`render(commands, config) -> RenderOutput { receipts, drawer_kicks }` do
everything in one call.

Receipts: a cut closes the current receipt and starts a new one with the same
printer settings (a cut is not a reset). A receipt with nothing on it (zero
height) is never produced. Only the paper is a receipt: text still in the
print buffer (no `LF` yet) is not in snapshots; it is printed by `finish()`.

Text is emitted only when a non-printable byte arrives, after 4096 bytes, or on
`finish()`, so the output never depends on how the stream was split.

Commands with binary data (`GS v 0`, `GS ( k`, `GS ( L`, `GS 8 L`, `ESC *`,
`GS *`, `GS k`) are framed by the lengths they declare and emitted once
complete; their data never leaks into the receipt as text. Memory is bounded:
a payload over `MAX_PAYLOAD` (1 MiB) is not stored; its command is emitted
with the header only and the data follows as `skippedData` items of at most
`SKIP_CHUNK` (64 KiB) bytes. `Parser::pending_bytes()` reports what the parser
holds.

## Command log JSON

Every log item:

```json
{ "offset": 0, "bytes": [27, 69, 1], "command": { "type": "setBold", "enabled": true } }
```

- `offset`: position of the first byte in the connection stream (u64).
- `bytes`: the exact bytes; concatenating all items reproduces the stream.
- `command.type`: one of the variants below. Other keys are the fields.
  `null` means the parameter is outside the Epson range: the command is
  logged and has no effect on the receipt.

| `type` | Fields | Bytes |
| --- | --- | --- |
| `text` | `text`: string (decoded with the active code page) | 0x20-0xFF |
| `initialize` | - | `ESC @` |
| `lineFeed` | - | `LF` |
| `carriageReturn` | - | `CR` |
| `horizontalTab` | - | `HT` |
| `feedLines` | `lines`: 0-255 | `ESC d n` |
| `feedDots` | `dots`: 0-255 | `ESC J n` |
| `resetLineSpacing` | - | `ESC 2` |
| `setLineSpacing` | `dots`: 0-255 | `ESC 3 n` |
| `setBold` | `enabled`: bool | `ESC E n` |
| `setUnderline` | `mode`: `"off"` \| `"oneDot"` \| `"twoDots"` \| null | `ESC - n` |
| `setReverse` | `enabled`: bool | `GS B n` |
| `setFont` | `font`: `"a"` \| `"b"` \| `"c"` \| `"d"` \| `"e"` \| `"specialA"` \| `"specialB"` \| null | `ESC M n` |
| `setCharacterSize` | `width`: 1-8 \| null, `height`: 1-8 \| null | `GS ! n` |
| `setPrintMode` | `font`: `"a"` \| `"b"`, `bold`, `doubleHeight`, `doubleWidth`, `underline`: bool | `ESC ! n` |
| `setAlignment` | `alignment`: `"left"` \| `"center"` \| `"right"` \| null | `ESC a n` |
| `setCodePage` | `page`: 0-255 (the `n`), `codePage`: `"cp437"` \| `"cp850"` \| `"cp860"` \| `"windows1252"` \| `"cp858"` \| null | `ESC t n` |
| `setInternationalCharset` | `n`: 0-255 | `ESC R n` |
| `cut` | `mode`: `"full"` \| `"partial"` \| null, `function`: `"a"` \| `"b"` \| `"c"` \| `"d"` \| null, `feed`: 0-255 \| null | `GS V m [n]`, `ESC i`, `ESC m` |
| `drawerKick` | `pin`: 2 \| 5 \| null, `onMs`: 0-510, `offMs`: 0-510 | `ESC p m t1 t2` |
| `rasterImage` | `mode`: `"normal"` \| `"doubleWidth"` \| `"doubleHeight"` \| `"quadruple"` \| null, `width` (dots, `x` x 8), `height` (dots), `bytesExpected` | `GS v 0 m xL xH yL yH d...` |
| `setQrModel` | `model`: `"model1"` \| `"model2"` \| `"micro"` \| null | `GS ( k` cn 49 fn 65 |
| `setQrModuleSize` | `size`: 1-16 \| null | `GS ( k` cn 49 fn 67 |
| `setQrErrorCorrection` | `level`: `"l"` \| `"m"` \| `"q"` \| `"h"` \| null | `GS ( k` cn 49 fn 69 |
| `qrStoreData` | `length` (data bytes), `valid`: bool | `GS ( k` cn 49 fn 80 |
| `qrPrint` | - | `GS ( k` cn 49 fn 81 |
| `qrRequestSize` | - (no reply yet) | `GS ( k` cn 49 fn 82 |
| `setBarcodeHeight` | `dots`: 1-255 \| null | `GS h n` |
| `setBarcodeWidth` | `module`: 2-6 \| null | `GS w n` |
| `setHriPosition` | `position`: `"none"` \| `"above"` \| `"below"` \| `"both"` \| null | `GS H n` |
| `setHriFont` | `font`: as `setFont` | `GS f n` |
| `barcode` | `symbology`, `function`: `"a"` \| `"b"` \| null, `data`: string, `valid`: bool, `problem`: string \| null | `GS k m ...` |
| `unsupportedImage` | `command`: string (e.g. `"GS ( L fn=112"`), `length` | `ESC *`, `GS ( L`, `GS 8 L`, `GS *`, `GS /`, `FS p`, `GS ( k` other than QR |
| `skippedData` | `length` | data of a payload over 1 MiB |
| `unknown` | `reason`: `"unsupportedCommand"` \| `"controlCharacter"` \| `"incomplete"` | anything else |

`cut`: `GS V` functions A (`m` 0/48 full, 1/49 partial), B (65 full, 66
partial: feed `n` dots, then cut), C (97/98: preset cutting position + `n`) and
D (103/104: feed `n`, cut, reverse feed); `feed` is `n` for B-D and null for A.
`ESC i` and `ESC m` are obsolete partial cuts (one and three points left uncut
in the Epson reference) and log as `{ mode: "partial", function: "a" }`. An `m`
outside that list is consumed as `GS V m` with every field null and ignored.
A cut is executed only at the beginning of a line: with data in the print
buffer it is logged and ignored (Epson notes for `GS V`). C is drawn as B; the
head-to-cutter distance is not modelled.

`barcode`: `symbology` is `"upcA"`, `"upcE"`, `"ean13"`, `"ean8"`, `"code39"`,
`"itf"`, `"codabar"`, `"code93"`, `"code128"` (drawn), or `"gs1128"`,
`"gs1DataBarOmnidirectional"`, `"gs1DataBarTruncated"`,
`"gs1DataBarLimited"`, `"gs1DataBarExpanded"`, `"code128Auto"` (logged with
`problem: "unsupportedSymbology"`), or null for an `m` in neither table
(`GS k m` only, `problem: "unknownSymbology"`). Function A (`m` 0-6) reads up
to NUL, at most 255 bytes (`"missingTerminator"` beyond); function B (`m`
65-79) reads `n` bytes. `data` is printable ASCII as is, other bytes as `\xNN`
and `\` as `\\`. Other `problem` values: `"invalidLength"`,
`"invalidCharacter"`, `"invalidStartStop"` (CODABAR), `"invalidNumberSystem"`
and `"notCompressible"` (UPC-E). UPC/EAN check digits are computed when
missing and printed as sent otherwise (the printer does not check them).
CODE128 data starts with `{A`, `{B` or `{C`; `{S` is SHIFT, `{1`-`{4` FNC1-4,
`{{` is `{`; in code set C each byte is a value 0-99. Encoding rules are in
`src/barcode.rs`.

QR Code: `qrStoreData` with `valid: false` (`m` not 48 or length outside
4-7092) stores nothing. Malformed QR functions (wrong `pL`) log as
`unsupportedImage`. Defaults: model 2, module 3 dots, level L.

`drawerKick`: `m` 0/48 is connector pin 2, 1/49 pin 5 (else `pin: null`, no
pulse). ON time is `t1 x 2` ms; OFF time is `t2 x 2` ms, or the ON time when
`t2 < t1` (reference note).

`unknown` reasons:

- `unsupportedCommand`: a prefix (`ESC`, `GS`, `DLE`, `FS`) and a command byte
  not implemented yet. Only those two bytes are consumed; parameters that
  follow are parsed as ordinary data until their stage implements them.
  Exceptions: every `GS ( x`, `ESC ( x` and `FS ( x` is consumed whole, by its
  `pL pH`; `GS v` not followed by `0` and `GS 8 x` other than `GS 8 L` take
  three bytes.
- `controlCharacter`: a lone byte 0x00-0x1F that is not a command (the
  printer ignores it).
- `incomplete`: the stream ended inside a command.

## Receipt JSON

```json
{
  "paper": "80mm", "width": 576, "height": 120, "cut": "full",
  "elements": [
    { "type": "line", "y": 0, "height": 24, "baseline": 21, "advance": 30,
      "alignment": "center", "contentWidth": 72,
      "spans": [
        { "text": "center", "x": 252, "y": 0, "width": 72, "columns": 6,
          "charWidth": 12, "charHeight": 24,
          "style": { "font": "a", "bold": false, "underline": "off", "reverse": false,
                     "widthMultiplier": 1, "heightMultiplier": 1 } }
      ] }
  ]
}
```

Images (`GS v 0`, QR Codes, barcodes) are `image` elements:

```json
{ "type": "image", "kind": "barcode", "x": 193, "y": 24, "width": 190,
  "height": 50, "advance": 50, "data": "<base64>" }
```

`kind` is `"raster"`, `"qrCode"` or `"barcode"`. `data` is standard base64
(with padding) of the bitmap rows, top to bottom, each `ceil(width / 8)` bytes,
most significant bit first, 1 = black dot; scaling (raster mode, QR module
size, `GS w`, `GS h`) is already applied. `x` includes the `ESC a` alignment
and `x + width` never exceeds the paper (raster images are clipped; barcodes
and QR Codes that do not fit are not printed). `advance` equals `height`. The
HRI of a barcode (`GS H`, font `GS f`) is ordinary `line` elements, one span
centred on the bars, above and/or below the image.

`cut` is how the receipt ended: `"full"` or `"partial"` (the cut that closed
it), or `null` when it was closed by the end of the stream or is still in
progress. All sizes are printer dots. A line starts at `y` and the next element at
`y + advance`; `span.x` already includes the alignment offset; `span.y` is the
top of the character cells inside the line (characters share the baseline).
`underline` is the effective one (never drawn under reverse text). Blank feeds
are lines with no spans. Fonts: A is 12x24 (baseline 21), B is 9x17
(baseline 16); paper is 384 dots (58 mm) or 576 dots (80 mm).

Drawer kick effect (`DrawerKick`): `{ "pin": 2, "onMs": 100, "offMs": 100 }`.

Layout rules and deliberate deviations from the Epson reference are documented
in `src/receipt.rs` and `src/parser.rs`.

## Code pages

Tables are generated by `tools/gen_codepages.py` from Python's codecs
(`python3 core/tools/gen_codepages.py > core/src/codepage_tables.rs`). The five
undefined Windows-1252 bytes print as spaces, as in Epson's page 16 table.
