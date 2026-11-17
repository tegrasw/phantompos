#!/usr/bin/env python3
"""Generate the `.bin` fixtures of phantompos-core.

Most fixtures are produced by python-escpos (Dummy printer), a widely used
ESC/POS library. Edge cases that no library emits are written as raw bytes and
are marked "HAND-WRITTEN" below.

Run it from anywhere, with python-escpos installed in a virtualenv OUTSIDE the
repository (see README.md in this directory). After regenerating, rebuild the
expected JSON files with:

    UPDATE_FIXTURES=1 cargo test -p phantompos-core --test fixtures

and review the diff by eye against the Epson reference.

Naming: `<name>.bin` is rendered on 80 mm paper; `<name>.58mm.bin` on 58 mm.
"""

from pathlib import Path

from escpos.printer import Dummy

OUT = Path(__file__).resolve().parent

ESC = b"\x1b"
GS = b"\x1d"
DLE = b"\x10"
FS = b"\x1c"


def write(name, data):
    (OUT / f"{name}.bin").write_bytes(bytes(data))


def printer():
    p = Dummy()
    p.hw("INIT")
    return p


# --------------------------------------------------------------------------
# Generated with python-escpos
# --------------------------------------------------------------------------


def plain_text():
    p = printer()
    p.text("Hello, PhantomPOS!\n")
    p.text("Second line 0123456789\n")
    p.text("Symbols: !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~\n")
    return p.output


def styles_bold_underline():
    p = printer()
    p.text("normal ")
    p.set(bold=True)
    p.text("bold")
    p.set(bold=False)
    p.text("\n")
    p.set(underline=1)
    p.text("underline 1 dot\n")
    p.set(underline=2)
    p.text("underline 2 dots\n")
    p.set(underline=0)
    p.text("no underline\n")
    return p.output


def styles_reverse_font_b():
    p = printer()
    p.set(invert=True)
    p.text(" REVERSE ")
    p.set(invert=False)
    p.text(" normal\n")
    p.set(font="b")
    p.text("Font B is 9x17 dots\n")
    p.set(font="a")
    p.text("Font A is 12x24 dots\n")
    p.set(invert=True, underline=1)
    p.text("reverse hides underline\n")
    return p.output


def styles_sizes():
    p = printer()
    p.set(double_width=True)
    p.text("Double width\n")
    p.set(double_width=False, double_height=True)
    p.text("Double height\n")
    p.set(double_width=True, double_height=True)
    p.text("Quad\n")
    p.set(normal_textsize=True)
    p.set(custom_size=True, width=3, height=2)
    p.text("3x2\n")
    p.set(custom_size=True, width=8, height=8)
    p.text("8x8\n")
    p.set(normal_textsize=True)
    p.text("normal again\n")
    return p.output


def alignment():
    p = printer()
    p.set(align="left")
    p.text("left\n")
    p.set(align="center")
    p.text("center\n")
    p.set(align="right")
    p.text("right\n")
    p.set(align="center", double_width=True)
    p.text("wide center\n")
    return p.output


def codepage(name, charcode, text):
    p = printer()
    p.charcode(charcode)
    p.text(text)
    return p.output


PT_TEXT = (
    "Ação, São Paulo, coração\n"
    "Café é ótimo; à vista\n"
    "1ª via, 2º andar\n"
    "Preço: € 9,90\n"
)


def line_feeds_spacing():
    p = printer()
    p.text("default spacing\n")
    p.ln(2)
    p.line_spacing(50)
    p.text("spacing 50\n")
    p.text("spacing 50\n")
    p.line_spacing()
    p.text("default again\n")
    p.print_and_feed(3)
    p.text("after ESC d 3\n")
    return p.output


LONG = (
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod "
    "tempor incididunt ut labore et dolore magna aliqua."
)


def wrapping():
    p = printer()
    p.text(LONG + "\n")
    p.set(double_width=True)
    p.text(LONG + "\n")
    p.set(normal_textsize=True, font="b")
    p.text(LONG + "\n")
    p.set(font="a")
    p.text("x" * 48 + "\n")
    return p.output


def receipt_small():
    p = printer()
    p.charcode("CP858")
    p.set(align="center", bold=True, double_height=True, double_width=True)
    p.text("PADARIA SÃO JOSÉ\n")
    p.set(normal_textsize=True, bold=False)
    p.text("Rua das Flores, 123 - Centro\n")
    p.text("CNPJ 12.345.678/0001-90\n")
    p.ln()
    p.set(align="left")
    p.text("Pão francês x10        R$ 8,50\n")
    p.text("Café coado             R$ 4,00\n")
    p.text("Pão de queijo x3       R$ 9,00\n")
    p.set(bold=True)
    p.text("TOTAL                 R$ 21,50\n")
    p.set(bold=False)
    p.ln()
    p.set(align="center", font="b")
    p.text("Obrigado pela preferência!\n")
    p.cut()
    return p.output


def cut_full():
    p = printer()
    p.text("full cut after ESC d 6\n")
    p.cut()  # ESC d 6, GS V 0
    return p.output


def cut_partial():
    p = printer()
    p.text("partial cut after ESC d 6\n")
    p.cut(mode="PART")  # ESC d 6, GS V 1
    return p.output


def cut_no_feed():
    p = printer()
    p.text("feed-and-cut, n = 0\n")
    p.cut(feed=False)  # GS V 66 0
    return p.output


def two_receipts():
    p = printer()
    p.set(align="center", bold=True)
    p.text("RECEIPT 1\n")
    p.cut()
    # Settings survive the cut: still centered and bold.
    p.text("RECEIPT 2\n")
    p.set(align="left", bold=False)
    p.text("left again\n")
    p.cut(mode="PART")
    return p.output


def three_receipts():
    p = printer()
    for i in range(1, 4):
        p.text(f"receipt {i}\n")
        p.cut()
    p.text("pending, closed by end of stream\n")
    return p.output


def drawer_kick():
    p = printer()
    p.text("open drawer\n")
    p.cashdraw(2)  # ESC p 0 '2' '2': 100 ms / 100 ms
    p.cashdraw(5)  # ESC p 1 '2' '2'
    p.cashdraw([27, 112, 48, 25, 250])  # ESC p '0' 25 250: 50 ms / 500 ms
    return p.output


def receipt_full():
    p = printer()
    p.charcode("CP858")
    p.set(align="center", bold=True, double_height=True, double_width=True)
    p.text("MERCADO BOA VISTA\n")
    p.set(normal_textsize=True, bold=False)
    p.text("Av. Brasil, 1000 - Centro\n")
    p.text("CNPJ 98.765.432/0001-10\n")
    p.text("-" * 32 + "\n")
    p.set(align="left")
    p.text("Arroz 5 kg             R$ 27,90\n")
    p.text("Feijão 1 kg             R$ 8,49\n")
    p.text("Açúcar 1 kg             R$ 4,99\n")
    p.set(bold=True)
    p.text("TOTAL                  R$ 41,38\n")
    p.set(bold=False)
    p.text("Dinheiro               R$ 50,00\n")
    p.text("Troco                   R$ 8,62\n")
    p.ln()
    p.set(align="center", font="b")
    p.text("Volte sempre!\n")
    p.cut()
    p.cashdraw(2)
    return p.output


# --------------------------------------------------------------------------
# HAND-WRITTEN raw bytes (edge cases no library emits)
# --------------------------------------------------------------------------


def print_mode_combos():
    # HAND-WRITTEN: ESC ! combinations and its interaction with ESC -.
    return (
        ESC + b"@"
        + ESC + b"!\x00" + b"plain\n"
        + ESC + b"!\x01" + b"font B\n"
        + ESC + b"!\x08" + b"bold\n"
        + ESC + b"!\x38" + b"bold quad\n"
        + ESC + b"!\x89" + b"B bold underline\n"
        # ESC - 2 then ESC - 0 keeps the 2-dot thickness; ESC ! bit 7 reuses it.
        + ESC + b"-\x02" + ESC + b"-\x00" + ESC + b"!\x80" + b"2-dot via ESC !\n"
        # GS ! after ESC ! overrides the size only.
        + ESC + b"!\x08" + GS + b"!\x10" + b"bold double width\n"
        + ESC + b"!\x00" + b"reset\n"
    )


def underline_ascii_params():
    # HAND-WRITTEN: ESC - and ESC a / ESC M with ASCII '0'..'2' parameters.
    return (
        ESC + b"-1" + b"one\n"
        + ESC + b"-2" + b"two\n"
        + ESC + b"-0" + b"off\n"
        + ESC + b"a1" + b"center\n"
        + ESC + b"a0"
        + ESC + b"M1" + b"font B\n"
        + ESC + b"M0" + b"font A\n"
    )


def mid_line_alignment():
    # HAND-WRITTEN: ESC a is only effective at the beginning of a line.
    return (
        b"start " + ESC + b"a\x02" + b"still left\n"
        + ESC + b"a\x02" + b"right\n"
    )


def unknown_sequences():
    # HAND-WRITTEN: commands outside stage 2 and stray control bytes.
    return (
        b"A" + ESC + b"G\x01" + b"B\n"  # ESC G (double-strike): unknown, 0x01 stray
        + ESC + b"{\x00" + b"C\n"  # ESC { (upside-down)
        + FS + b"p\x01\x00" + b"D\n"  # FS p (NV bit image)
        + b"\x00\x07E\n"  # NUL and BEL: ignored control bytes
        + ESC + ESC + b"@F\n"  # ESC ESC: unknown, then '@' is text
        + b"\x0cG\x18\n"  # FF and CAN (not implemented)
    )


def later_stage_commands():
    # HAND-WRITTEN: stage 3-5 commands; only prefix + command byte is unknown,
    # parameters fall through as data until their stage implements them.
    return (
        b"before cut\n"
        + GS + b"V\x42\x00"  # GS V 66 0: feed and partial cut
        + ESC + b"i" + ESC + b"m"  # ESC i / ESC m: cuts
        + ESC + b"p\x00\x19\xfa"  # ESC p 0 25 250: drawer kick
        + GS + b"v0\x00\x01\x00\x01\x00\xff"  # GS v 0: 8x1 raster
        + GS + b"(k\x03\x001A2"  # GS ( k: QR model
        + GS + b"h\x50" + GS + b"w\x02" + GS + b"H\x02"
        + GS + b"k\x02" + b"7891234567895\x00"  # GS k: EAN-13
        + DLE + b"\x04\x01"  # DLE EOT 1
        + GS + b"r\x01" + GS + b"a\xff"
        + b"\nafter\n"
    )


def out_of_range_params():
    # HAND-WRITTEN: parameters outside the ranges in the Epson reference.
    return (
        ESC + b"-\x03" + b"underline n=3 ignored\n"
        + ESC + b"M\x05" + b"font n=5 ignored\n"
        + ESC + b"M\x02" + b"font C not drawn\n"
        + GS + b"!\x88" + b"size n=0x88 ignored\n"
        + ESC + b"a\x03" + b"align n=3 ignored\n"
        + ESC + b"t\x63" + b"page 99 \x80\x87 decodes as CP437\n"
        + ESC + b"E\xff" + b"bold n=255 (LSB 1)\n"
        + ESC + b"E\xfe" + b"bold n=254 (LSB 0)\n"
        + ESC + b"R\x0c" + b"ESC R 12 logged only\n"
    )


def incomplete_eof():
    # HAND-WRITTEN: stream ends inside a command (ESC 3 without n).
    return b"pending text" + ESC + b"3"


def incomplete_eof_prefix():
    # HAND-WRITTEN: stream ends right after a prefix byte.
    return b"line\n" + GS


def unterminated_text():
    # HAND-WRITTEN: last line has no LF; it is printed at end of stream.
    return b"first\nno newline at end"


def cr_lf_combos():
    # HAND-WRITTEN: CR is ignored (auto line feed off); LF feeds.
    return (
        b"crlf\r\n"
        + b"lfcr\n\r"
        + b"cr only\r"
        + b"same line\n"
        + b"double cr\r\r\n"
        + b"\n\n"
        + b"end\n"
    )


def tabs():
    # HAND-WRITTEN: HT with the default stops (every 8 font-A columns).
    return (
        b"a\tb\tc\n"
        + b"\tindented\n"
        + b"123456789\tx\n"
        + b"\t\t\t\t\tlast stop wraps\n"
        + b"\t\t\t\t\t\t\twraps\n"
    )


def initialize_reset():
    # HAND-WRITTEN: ESC @ clears the print buffer and resets every mode.
    return (
        ESC + b"E\x01" + ESC + b"-\x01" + GS + b"B\x01" + GS + b"!\x11"
        + ESC + b"a\x01" + ESC + b"3\x50" + ESC + b"t\x02"
        + b"styled \xc6\n"  # 0xC6 is a-tilde in CP850
        + b"discarded by ESC @" + ESC + b"@"
        + b"plain \xc6\n"  # back to CP437: box drawing
        + b"plain\n"
    )


def raw_feeds():
    # HAND-WRITTEN: ESC J and ESC d with and without pending data.
    return (
        b"J 100" + ESC + b"J\x64"
        + ESC + b"J\x00"
        + b"tall" + GS + b"!\x03" + b"T" + ESC + b"J\x0a"
        + GS + b"!\x00"
        + ESC + b"d\x00"
        + b"d 0" + ESC + b"d\x00"
        + ESC + b"3\x00" + b"spacing 0\nspacing 0\n"
        + ESC + b"2" + b"end\n"
    )


def cut_functions():
    # HAND-WRITTEN: every GS V function (A-D, full and partial) and the
    # obsolete ESC i / ESC m (both partial cuts in the Epson reference).
    out = b""
    for m, n in [(0, None), (1, None), (48, None), (49, None), (65, 24),
                 (66, 0), (97, 10), (98, 10), (103, 5), (104, 5)]:
        out += f"GS V {m}\n".encode() + GS + b"V" + bytes([m])
        if n is not None:
            out += bytes([n])
    out += b"ESC i\n" + ESC + b"i"
    out += b"ESC m\n" + ESC + b"m"
    return out


def cut_empty():
    # HAND-WRITTEN: cuts with nothing on the paper produce no receipt; a
    # feed-only receipt has height and is produced.
    return (
        GS + b"V\x00"  # at power on
        + ESC + b"i" + GS + b"V\x01"  # several in a row
        + ESC + b"@" + GS + b"V\x00"  # after a reset
        + GS + b"VA\x00"  # function B with n = 0: no feed
        + GS + b"VA\x28"  # function B with n = 40: a 40-dot blank receipt
        + b"text\n" + GS + b"V\x00"
    )


def cut_mid_line():
    # HAND-WRITTEN: a cut is only executed at the beginning of a line; with
    # data in the print buffer (text or an HT skip) it is ignored.
    return (
        b"before " + GS + b"V\x00" + b"same line\n"
        + b"\t" + ESC + b"i" + b"after tab\n"
        + GS + b"VB\x10"  # beginning of line: executed
        + b"no LF before EOF" + GS + b"V\x01"  # ignored; printed at EOF
    )


def drawer_kick_params():
    # HAND-WRITTEN: ESC p with ASCII pins, t2 < t1 (OFF time = ON time),
    # zero times, an out-of-range pin (no pulse) and a kick mid-line.
    return (
        ESC + b"p1\x05\x0a"
        + ESC + b"p\x00\x64\x0a"  # t2 < t1
        + ESC + b"p\x01\x00\x00"
        + ESC + b"p\x02\x19\xfa"  # m = 2: out of range
        + b"mid" + ESC + b"p0\xff\xff" + b"line\n"
    )


def cut_drawer_out_of_range():
    # HAND-WRITTEN: GS V with m outside the table (taken as GS V m, ignored),
    # and an invalid ESC p pin.
    return (
        b"one\n"
        + GS + b"V\x02"
        + GS + b"V\xff"
        + GS + b"V\x63" + b"x\n"  # 99 is not function C (97/98): 'x' is text
        + ESC + b"p\x33\x10\x10"  # '3': out of range
        + b"two\n"
    )


def incomplete_cut_eof():
    # HAND-WRITTEN: stream ends inside GS V function B (n missing).
    return b"line\n" + GS + b"VA"


def incomplete_drawer_eof():
    # HAND-WRITTEN: stream ends inside ESC p (t2 missing).
    return b"line\n" + ESC + b"p\x00\x19"


def utf8_bytes():
    # HAND-WRITTEN: UTF-8 sent to a CP437 printer prints mojibake, as on paper.
    return "Ação\n".encode("utf-8")


FIXTURES = {
    "plain_text": plain_text,
    "styles_bold_underline": styles_bold_underline,
    "styles_reverse_font_b": styles_reverse_font_b,
    "styles_sizes": styles_sizes,
    "alignment": alignment,
    "alignment.58mm": alignment,
    "codepage_cp437": lambda: codepage("cp437", "CP437", PT_TEXT),
    "codepage_cp850": lambda: codepage("cp850", "CP850", PT_TEXT),
    "codepage_cp860": lambda: codepage("cp860", "CP860", PT_TEXT),
    "codepage_cp858": lambda: codepage("cp858", "CP858", PT_TEXT),
    "codepage_wpc1252": lambda: codepage("wpc1252", "CP1252", PT_TEXT),
    "line_feeds_spacing": line_feeds_spacing,
    "wrapping": wrapping,
    "wrapping.58mm": wrapping,
    "receipt_small": receipt_small,
    "receipt_small.58mm": receipt_small,
    "print_mode_combos": print_mode_combos,
    "underline_ascii_params": underline_ascii_params,
    "mid_line_alignment": mid_line_alignment,
    "unknown_sequences": unknown_sequences,
    "later_stage_commands": later_stage_commands,
    "out_of_range_params": out_of_range_params,
    "incomplete_eof": incomplete_eof,
    "incomplete_eof_prefix": incomplete_eof_prefix,
    "unterminated_text": unterminated_text,
    "cr_lf_combos": cr_lf_combos,
    "tabs": tabs,
    "initialize_reset": initialize_reset,
    "raw_feeds": raw_feeds,
    "utf8_bytes": utf8_bytes,
    "empty": lambda: b"",
    "cut_full": cut_full,
    "cut_partial": cut_partial,
    "cut_no_feed": cut_no_feed,
    "two_receipts": two_receipts,
    "three_receipts": three_receipts,
    "drawer_kick": drawer_kick,
    "receipt_full": receipt_full,
    "receipt_full.58mm": receipt_full,
    "cut_functions": cut_functions,
    "cut_empty": cut_empty,
    "cut_mid_line": cut_mid_line,
    "drawer_kick_params": drawer_kick_params,
    "cut_drawer_out_of_range": cut_drawer_out_of_range,
    "incomplete_cut_eof": incomplete_cut_eof,
    "incomplete_drawer_eof": incomplete_drawer_eof,
}


def main():
    for name, build in FIXTURES.items():
        write(name, build())
    print(f"wrote {len(FIXTURES)} fixtures to {OUT}")


if __name__ == "__main__":
    main()
