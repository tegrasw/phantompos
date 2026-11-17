# Parser fixtures

Each case is a pair `<name>.bin` (raw bytes as sent to port 9100) and
`<name>.json` (expected output):

```json
{ "paper": "80mm", "commands": [ ...log items... ], "receipt": { ... } }
```

- `<name>.58mm.bin` is rendered on 58 mm paper; every other `.bin` on 80 mm.
  The JSON repeats the profile in `"paper"`.
- Default parser settings: CP437 as the default code page, `CR` ignored.
- `tests/fixtures.rs` checks every pair; a `.bin` without `.json` fails.
- `tests/fragmentation.rs` replays every `.bin` whole, byte by byte and in
  other chunkings, and requires identical results.

## Regenerating the `.bin` files

The bytes come from [python-escpos](https://github.com/python-escpos/python-escpos)
(Dummy printer) plus hand-written edge cases, all in `generate.py`. Install the
library in a virtualenv **outside** the repository:

```bash
python3 -m venv ~/.venvs/phantompos-fixtures
~/.venvs/phantompos-fixtures/bin/pip install python-escpos   # 3.1 was used
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
| `receipt_small`, `receipt_small.58mm` | python-escpos | realistic receipt in CP858 ending with a cut (`GS V` is unknown until stage 3) |
| `print_mode_combos` | hand-written | `ESC !` bits; underline thickness kept across `ESC - 0`; `GS !` after `ESC !` |
| `underline_ascii_params` | hand-written | ASCII `'0'..'2'` parameters of `ESC -`, `ESC a`, `ESC M` |
| `mid_line_alignment` | hand-written | `ESC a` ignored when not at the beginning of a line |
| `unknown_sequences` | hand-written | unimplemented `ESC`/`FS` commands, NUL/BEL/FF/CAN, `ESC ESC` |
| `later_stage_commands` | hand-written | stage 3-5 commands appear as unknown; their parameters fall through as data |
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
