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

`drawerKick`: `m` 0/48 is connector pin 2, 1/49 pin 5 (else `pin: null`, no
pulse). ON time is `t1 x 2` ms; OFF time is `t2 x 2` ms, or the ON time when
`t2 < t1` (reference note).

`unknown` reasons:

- `unsupportedCommand`: a prefix (`ESC`, `GS`, `DLE`, `FS`) and a command byte
  not implemented yet. Only those two bytes are consumed; parameters that
  follow are parsed as ordinary data until their stage implements them.
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
