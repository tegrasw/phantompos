# Parser fixtures

Each case is a pair `<name>.bin` (raw bytes as sent to port 9100) and
`<name>.json` (expected output):

```json
{ "paper": "80mm", "commands": [ ...log items... ], "receipts": [ ... ], "drawerKicks": [ ... ] }
```

Three more keys appear only when they are not empty: `"status"` (the
simulated state, when not the normal one), `"replies"` (every reply the
printer sent, in order: `{ request, kind, bytes, flags }`, see the core
README) and `"suppressed"` (`{ reason, bytes }`: total bytes discarded
because they arrived in an error state).

`receipts` lists every receipt in closing order (one per cut); the last one,
not cut (`"cut": null`), is included when the stream ended with content on it.
`drawerKicks` lists the `ESC p` pulses (`{ pin, onMs, offMs }`).

- `<name>.58mm.bin` is rendered on 58 mm paper; every other `.bin` on 80 mm.
  The JSON repeats the profile in `"paper"`.
- `<name>.<states>.bin`, where `<states>` is `paperOut`, `coverOpen`,
  `offline` and/or `drawerOpen` joined by `+` (for example
  `status_requests.coverOpen+paperOut.bin`), runs the whole stream in that
  simulated state; no tag is the normal state. A drawer kick in the stream
  opens the drawer for the rest of it, as in the app. Unknown tags fail the
  test. Changing the state in the middle of a stream is covered by unit
  tests (`src/session.rs`) and the backend tests, not by fixtures.
- Default parser settings: CP437 as the default code page, `CR` ignored.
- `tests/fixtures.rs` checks every pair; a `.bin` without `.json` fails.
- `tests/fragmentation.rs` replays every `.bin` whole, byte by byte and in
  other chunkings, and requires identical results (commands, receipts,
  drawer kicks, replies and discarded bytes).

## Regenerating the `.bin` files

The bytes come from [python-escpos](https://github.com/python-escpos/python-escpos)
(Dummy printer) plus hand-written edge cases, all in `generate.py`. Install the
library in a virtualenv **outside** the repository:

```bash
python3 -m venv ~/.venvs/phantompos-fixtures  # any directory outside the repo
~/.venvs/phantompos-fixtures/bin/pip install python-escpos pillow   # 3.1 / 13.0 were used
~/.venvs/phantompos-fixtures/bin/python core/tests/fixtures/generate.py
```

## Updating the expected JSON

```bash
UPDATE_FIXTURES=1 cargo test -p phantompos-core --test fixtures
```

Review every changed JSON against the Epson reference before committing: a
fixture that enshrines a bug is worse than none.

## Cases

| Fixture | Source | What it covers |
| --- | --- | --- |
| `plain_text` | python-escpos | ASCII text and LF |
| `styles_bold_underline` | python-escpos | `ESC E`, `ESC -` 1 and 2 dots |
| `styles_reverse_font_b` | python-escpos | `GS B`, `ESC M`; no underline on reverse text |
| `styles_sizes` | python-escpos | double width/height via `ESC !`, `GS !` 3x2 and 8x8 |
| `alignment`, `alignment.58mm` | python-escpos | `ESC a` left/center/right, wide centered text |
| `codepage_cp437/cp850/cp860/cp858/wpc1252` | python-escpos | `ESC t` with Portuguese text (ç, ã, é, ª, º, €) |
| `line_feeds_spacing` | python-escpos | blank LFs, `ESC 3`, `ESC 2`, `ESC d` |
| `wrapping`, `wrapping.58mm` | python-escpos | auto line feed at 32/48 columns, double width, font B |
| `receipt_small`, `receipt_small.58mm` | python-escpos | realistic receipt in CP858 ending with `ESC d 6` + `GS V 0` |
| `receipt_full`, `receipt_full.58mm` | python-escpos | realistic receipt ending with feed + full cut + drawer kick |
| `cut_full`, `cut_partial` | python-escpos | `cut()`: `ESC d 6` then `GS V 0` / `GS V 1` |
| `cut_no_feed` | python-escpos | `cut(feed=False)`: `GS V 66 0` |
| `two_receipts` | python-escpos | two receipts; alignment and bold survive the cut |
| `three_receipts` | python-escpos | three cut receipts plus a pending one closed by end of stream |
| `drawer_kick` | python-escpos | `cashdraw(2)`, `cashdraw(5)`, custom `ESC p '0' 25 250` |
| `print_mode_combos` | hand-written | `ESC !` bits; underline thickness kept across `ESC - 0`; `GS !` after `ESC !` |
| `underline_ascii_params` | hand-written | ASCII `'0'..'2'` parameters of `ESC -`, `ESC a`, `ESC M` |
| `mid_line_alignment` | hand-written | `ESC a` ignored when not at the beginning of a line |
| `unknown_sequences` | hand-written | unimplemented `ESC` commands, `FS p` (consumed as `unsupportedImage`), NUL/BEL/FF/CAN, `ESC ESC` |
| `later_stage_commands` | hand-written | stage 3-5 commands in one stream, all decoded: cut, drawer, raster, QR (wrong length), barcode, `DLE EOT 1`, `GS r 1`, `GS a 255` (with their replies) |
| `cut_functions` | hand-written | every `GS V` function (A-D, full/partial) and `ESC i` / `ESC m` |
| `cut_empty` | hand-written | cuts with nothing printed produce no receipt; a feed-only (`GS V 65 40`) receipt is produced |
| `cut_mid_line` | hand-written | a cut with data in the print buffer (text, HT) is ignored; the data continues the receipt |
| `drawer_kick_params` | hand-written | ASCII pin, `t2 < t1`, zero times, out-of-range pin, kick mid-line |
| `cut_drawer_out_of_range` | hand-written | `GS V` with `m` outside the table, `ESC p` with an invalid pin |
| `incomplete_cut_eof` | hand-written | stream ends inside `GS V 65` (no `n`) |
| `incomplete_drawer_eof` | hand-written | stream ends inside `ESC p` |
| `out_of_range_params` | hand-written | out-of-range `ESC -`, `ESC M`, `GS !`, `ESC a`, `ESC t`; LSB of `ESC E`; `ESC R` |
| `incomplete_eof` | hand-written | stream ends inside `ESC 3` |
| `incomplete_eof_prefix` | hand-written | stream ends after a lone `GS` |
| `unterminated_text` | hand-written | last line without LF is printed at end of stream |
| `cr_lf_combos` | hand-written | CRLF, LFCR, lone CR (ignored), double CR |
| `tabs` | hand-written | HT with default stops, last stop, wrap past the right edge |
| `initialize_reset` | hand-written | `ESC @` discards the unprinted line and resets modes and code page |
| `raw_feeds` | hand-written | `ESC J`/`ESC d` with and without data, tall characters, zero spacing |
| `utf8_bytes` | hand-written | UTF-8 bytes decoded as CP437 (mojibake, as a printer prints it) |
| `empty` | hand-written | empty stream |
| `image_raster_logo`, `image_raster_logo.58mm` | python-escpos | centred `GS v 0` logo drawn with PIL in `generate.py` |
| `image_alignment` | python-escpos | `GS v 0` left/center/right; double width, double height, quadruple |
| `image_wider_than_paper`, `.58mm` | python-escpos | 640-dot image clipped to 576 / 384 dots at the left edge |
| `qr_codes` | python-escpos | native QR: defaults, URL size 6 level M, UTF-8 Portuguese size 4 level Q centred, numeric size 2 level H right |
| `qr_models` | python-escpos | model 1 (drawn as model 2) and Micro QR |
| `barcode_ean13` | python-escpos | EAN-13 function A (check digit added) and B (sent) |
| `barcode_symbologies_a` / `_b` | python-escpos | every drawn symbology in function A / B (CODE93, CODE128 with `{A`/`{B`/`{C` in B only) |
| `barcode_hri` | python-escpos | HRI none/above/below/both, fonts A/B, heights, module widths 2-6, alignment |
| `receipt_codes`, `receipt_codes.58mm` | python-escpos | stage 4 acceptance example: logo, items, EAN-13, QR Code, cut |
| `unsupported_graphics` | python-escpos | `image(impl="graphics")` (`GS ( L` fn 112/50) and `impl="bitImageColumn"` (`ESC *`): consumed, not drawn |
| `image_edge_cases` | hand-written | zero-size images, ASCII mode, invalid mode, image mid-line (ignored), right alignment |
| `image_truncated_eof` | hand-written | stream ends inside `GS v 0` data |
| `image_huge_declared` | hand-written | 65535 x 65535 declared: header emitted, data skipped, stream ends early |
| `barcode_invalid` | hand-written | invalid data per symbology, wrong check digit (printed), GS1 (not drawn), unknown `m`, `n = 0`, function A without NUL |
| `barcode_settings` | hand-written | `GS h`/`GS w`/`GS H`/`GS f` out of range, ASCII values, font C ignored, `ESC @` defaults, too wide |
| `qr_edge_cases` | hand-written | print with no data, `m != 48`, invalid parameters, wrong length, too long for level H, too wide, mid-line, `ESC @` |
| `unsupported_symbols` | hand-written | PDF417, MaxiCode, DataMatrix, `GS 8 L`, `GS *`, `GS /`, `FS p`, `GS ( E`, `ESC *` with an unknown mode |
| `incomplete_qr_eof` | hand-written | stream ends inside `GS ( k` fn 80 |
| `status_requests` + `.drawerOpen`, `.paperOut`, `.coverOpen`, `.offline`, `.coverOpen+paperOut`, `.coverOpen+drawerOpen+offline+paperOut` | python-escpos | `query_status`/`is_online` (`DLE EOT 1`), `paper_status` (`DLE EOT 4`), plus `DLE EOT 2`/`3`, `GS r 1`/`49`/`2`/`50`, `DLE DC4 7 1`, text and a cut: every reply in every state; nothing printed offline |
| `status_asb`, `status_asb.paperOut` | hand-written | `GS a` 2, 0, reserved bits only, 255, `ESC @` (disables; discarded offline), ASB on the connection's own drawer kicks |
| `status_asb_kick`, `status_asb_kick.offline` | hand-written | drawer item enabled, `ESC p` (discarded offline) and `DLE DC4 1` kick, then `GS r 2` |
| `status_inside_data` | hand-written | `DLE EOT`, `GS r`, `DLE DC4 8` bytes inside `GS v 0` data, QR data, barcode data and as `ESC 3`'s parameter are not requests; one real `DLE EOT 1` |
| `status_realtime_functions`, `.coverOpen` | hand-written | `DLE DC4` 1 (valid and invalid `m`/`t`), 2, 3, 7 (`m` 1, 2, 4, 5), 8 (valid: clears the pending line; invalid), unknown `fn`; `DLE ENQ` 0-3 |
| `status_invalid` | hand-written | `DLE EOT` with `n`/`a` outside the reference, ink/peeler/interface/DM-D (not answered), `GS r` 0, 3, 4, 52, 255, unknown `DLE` command |
| `print_suppressed.paperOut`, `.coverOpen`, `.offline` | python-escpos | a full receipt (styles, raster image, QR Code, EAN-13, cut, drawer kick) in each error state: logged, `suppressed`, no receipt or kick |
| `incomplete_status_eof` | hand-written | stream ends inside `DLE EOT 7` (no `a`) |
| `incomplete_dc4_eof` | hand-written | stream ends inside `DLE DC4 8` |
