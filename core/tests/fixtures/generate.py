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
A tag of printer states joined by `+` (`<name>.coverOpen+paperOut.bin`) runs
the fixture in that simulated state (see README.md).
"""

from pathlib import Path

from escpos.constants import RT_STATUS_ONLINE, RT_STATUS_PAPER
from escpos.printer import Dummy
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent

ESC = b"\x1b"
GS = b"\x1d"
DLE = b"\x10"
FS = b"\x1c"


class StatusDummy(Dummy):
    """Dummy printer whose status queries get no answer (there is no
    printer); only the request bytes written by python-escpos matter."""

    def _read(self):
        return b""


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
# Stage 4: images, QR Codes and barcodes (python-escpos)
# --------------------------------------------------------------------------


def logo(scale=1):
    """PhantomPOS logo drawn with PIL primitives (deterministic, no files):
    a ghost and a blocky "POS" wordmark, 152 x 64 dots at scale 1."""
    w, h = 152 * scale, 64 * scale
    im = Image.new("1", (w, h), 1)  # white
    d = ImageDraw.Draw(im)
    s = scale
    # Ghost: round head, straight body, scalloped hem.
    d.ellipse((4 * s, 2 * s, 56 * s, 54 * s), fill=0)
    d.rectangle((4 * s, 28 * s, 56 * s, 54 * s), fill=0)
    for i in range(4):
        x0 = (4 + i * 13) * s
        d.ellipse((x0, 46 * s, x0 + 13 * s, 62 * s), fill=0)
    for i in range(3):
        x0 = (10 + i * 13) * s
        d.ellipse((x0 + 4 * s, 54 * s, x0 + 11 * s, 63 * s), fill=1)
    # Eyes.
    d.ellipse((17 * s, 18 * s, 27 * s, 32 * s), fill=1)
    d.ellipse((34 * s, 18 * s, 44 * s, 32 * s), fill=1)
    d.ellipse((21 * s, 23 * s, 26 * s, 30 * s), fill=0)
    d.ellipse((38 * s, 23 * s, 43 * s, 30 * s), fill=0)
    # "POS" in 6-dot strokes.
    t = 6 * s
    top, bottom = 12 * s, 52 * s
    mid = 32 * s
    # P
    x = 66 * s
    d.rectangle((x, top, x + t, bottom), fill=0)
    d.rectangle((x, top, x + 22 * s, top + t), fill=0)
    d.rectangle((x + 16 * s, top, x + 22 * s, mid), fill=0)
    d.rectangle((x, mid - t, x + 22 * s, mid), fill=0)
    # O
    x = 94 * s
    d.rectangle((x, top, x + 22 * s, bottom), fill=0)
    d.rectangle((x + t, top + t, x + 16 * s, bottom - t), fill=1)
    # S
    x = 122 * s
    d.rectangle((x, top, x + 24 * s, top + t), fill=0)
    d.rectangle((x, top, x + t, mid), fill=0)
    d.rectangle((x, mid - t // 2, x + 24 * s, mid + t // 2), fill=0)
    d.rectangle((x + 18 * s, mid, x + 24 * s, bottom), fill=0)
    d.rectangle((x, bottom - t, x + 24 * s, bottom), fill=0)
    return im


def checker(w, h, cell=4):
    """A small checkerboard, for alignment and scaling cases."""
    im = Image.new("1", (w, h), 1)
    d = ImageDraw.Draw(im)
    for y in range(0, h, cell):
        for x in range(0, w, cell):
            if (x // cell + y // cell) % 2 == 0:
                d.rectangle((x, y, x + cell - 1, y + cell - 1), fill=0)
    d.rectangle((0, 0, w - 1, h - 1), outline=0)
    return im


def image_raster_logo():
    p = printer()
    p.set(align="center")
    p.image(logo(), impl="bitImageRaster")
    p.text("PhantomPOS\n")
    return p.output


def image_alignment():
    p = printer()
    for align in ("left", "center", "right"):
        p.set(align=align)
        p.image(checker(40, 16), impl="bitImageRaster")
    # python-escpos low density flags select GS v 0 m = 1, 2 and 3.
    p.set(align="center")
    p.image(checker(24, 8), impl="bitImageRaster", high_density_horizontal=False)
    p.image(checker(24, 8), impl="bitImageRaster", high_density_vertical=False)
    p.image(
        checker(24, 8),
        impl="bitImageRaster",
        high_density_horizontal=False,
        high_density_vertical=False,
    )
    p.text("after images\n")
    return p.output


def image_wider_than_paper():
    # 640 dots on 80 mm (576) and 58 mm (384): clipped on the right.
    p = printer()
    p.set(align="center")
    p.image(checker(640, 16, cell=8), impl="bitImageRaster")
    p.text("clipped\n")
    return p.output


def qr_codes():
    p = printer()
    p.text("QR model 2, size 3, level L (defaults)\n")
    p.qr("PhantomPOS", native=True)
    p.text("size 6, level M, URL\n")
    p.qr("https://github.com/tegrasw/phantompos", size=6, ec=1, native=True)
    p.set(align="center")
    p.text("centred, size 4, level Q, UTF-8\n")
    p.qr("Pão de queijo — R$ 9,90 · Ação", size=4, ec=2, native=True)
    p.set(align="right")
    p.text("right, size 2, level H, numeric\n")
    p.qr("0123456789012345678901234567890123456789", size=2, ec=3, native=True)
    p.set(align="left")
    return p.output


def qr_models():
    p = printer()
    p.text("model 1 (drawn as model 2)\n")
    p.qr("MODEL 1", model=1, native=True)
    p.text("micro QR\n")
    p.qr("12345", model=3, size=4, native=True)
    return p.output


def barcode_ean13():
    p = printer()
    p.text("EAN-13, check digit added (function A)\n")
    p.barcode("789123456789", "EAN13", function_type="A")
    p.text("EAN-13 with check digit (function B)\n")
    p.barcode("7891234567895", "EAN13", function_type="B")
    return p.output


def barcode_symbologies(function_type):
    p = printer()
    cases = [
        ("UPC-A", "03600029145"),
        ("UPC-E", "01234565"),
        ("EAN8", "9638507"),
        ("CODE39", "PHANTOM-42"),
        ("ITF", "12345678"),
        ("NW7", "A40156B"),
    ]
    if function_type == "B":
        cases += [
            ("CODE93", "PhantomPOS 93"),
            ("CODE128", "{BPhantom-128"),
            ("CODE128", "{C" + "".join(chr(v) for v in (12, 34, 56, 78))),
            ("CODE128", "{AABC{B-abc{C" + chr(99) + chr(1)),
        ]
    for bc, code in cases:
        p.set(align="left")
        p.text(f"{bc} {function_type}\n")
        p.barcode(code, bc, height=60, width=2, pos="BELOW", function_type=function_type)
    return p.output


def barcode_hri():
    p = printer()
    for pos in ("OFF", "ABOVE", "BELOW", "BOTH"):
        p.barcode("4006381333931", "EAN13", height=40, width=2, pos=pos, font="A")
    p.barcode("4006381333931", "EAN13", height=40, width=2, pos="BOTH", font="B")
    p.set(align="left")
    p.barcode("CODE39", "CODE39", height=80, width=3, pos="BELOW", align_ct=False)
    p.barcode("12", "CODE39", height=20, width=6, pos="BELOW", align_ct=False)
    p.set(align="right")
    p.barcode("1234", "ITF", height=30, width=4, pos="ABOVE", align_ct=False)
    return p.output


def receipt_codes():
    # The stage 4 acceptance example: logo, items, EAN-13, QR Code, cut.
    p = printer()
    p.charcode("CP858")
    p.set(align="center")
    p.image(logo(), impl="bitImageRaster")
    p.set(align="center", bold=True, double_height=True, double_width=True)
    p.text("PADARIA SÃO JOSÉ\n")
    p.set(normal_textsize=True, bold=False)
    p.text("CNPJ 12.345.678/0001-90\n")
    p.text("-" * 32 + "\n")
    p.set(align="left")
    p.text("Pão francês x10        R$ 8,50\n")
    p.text("Café coado             R$ 4,00\n")
    p.text("Pão de queijo x3       R$ 9,00\n")
    p.set(bold=True)
    p.text("TOTAL                 R$ 21,50\n")
    p.set(bold=False)
    p.ln()
    p.barcode("789123456789", "EAN13", height=64, width=2, pos="BELOW", font="A")
    p.ln()
    p.set(align="center")
    p.text("Consulte pelo QR Code\n")
    p.qr("https://www.sefaz.example/nfce?p=35261112345678000190650010000012341000012345|2|1|1|ABCDEF0123456789",
         size=4, ec=1, native=True)
    p.set(align="center", font="b")
    p.text("Obrigado pela preferência!\n")
    p.cut()
    return p.output


def unsupported_graphics():
    # python-escpos "graphics" (GS ( L fn 112 + fn 50) and "bitImageColumn"
    # (ESC *) implementations: recognized by length, not drawn.
    p = printer()
    p.text("graphics\n")
    p.image(checker(16, 8), impl="graphics")
    p.text("bitImageColumn\n")
    p.image(checker(16, 8), impl="bitImageColumn")
    p.text("after\n")
    return p.output


# --------------------------------------------------------------------------
# HAND-WRITTEN raw bytes (edge cases no library emits)
# --------------------------------------------------------------------------


def raster(m, width_bytes, height, data=None):
    if data is None:
        data = bytes([0xF0, 0x0F]) * ((width_bytes * height) // 2) + (
            b"\xff" if (width_bytes * height) % 2 else b""
        )
    return (
        GS + b"v0" + bytes([m])
        + bytes([width_bytes & 0xFF, width_bytes >> 8, height & 0xFF, height >> 8])
        + data
    )


def gs_k_a(m, data):
    return GS + b"k" + bytes([m]) + data + b"\x00"


def gs_k_b(m, data):
    return GS + b"k" + bytes([m, len(data)]) + data


def qr_fn(fn, params):
    body = b"1" + bytes([fn]) + params
    return GS + b"(k" + bytes([len(body) & 0xFF, len(body) >> 8]) + body


def image_edge_cases():
    # HAND-WRITTEN: zero-size images, ASCII mode, invalid mode, an image
    # sent mid-line (ignored), and ESC @ restoring defaults.
    return (
        b"zero width" + b"\n" + raster(0, 0, 10, b"")
        + b"zero height\n" + raster(0, 4, 0, b"")
        + b"ASCII '3' quadruple\n" + raster(ord("3"), 1, 2, b"\x81\x7e")
        + b"mode 7 consumed, not drawn\n" + raster(7, 1, 2, b"\xff\xff")
        + b"mid-line " + raster(0, 1, 1, b"\xff") + b"ignored\n"
        + ESC + b"a\x02" + raster(48, 2, 2, b"\xff\x01\x80\xff")
        + b"end\n"
    )


def image_truncated_eof():
    # HAND-WRITTEN: stream ends inside GS v 0 data (10 x 10 declared, 20 sent).
    return b"before\n" + GS + b"v0\x00\x0a\x00\x0a\x00" + b"\xaa" * 20


def image_huge_declared():
    # HAND-WRITTEN: GS v 0 declaring 65535 x 65535 bytes (> 1 MiB): the header
    # is emitted at once, the data is skipped; the stream ends early.
    return b"huge\n" + GS + b"v0\x00\xff\xff\xff\xff" + b"\x55" * 100


def barcode_invalid():
    # HAND-WRITTEN: data the reference rejects (logged, nothing drawn) and a
    # wrong check digit (printed as sent: the printer does not check it).
    return (
        GS + b"H\x02"
        + b"EAN-13 wrong check digit (printed)\n" + gs_k_a(2, b"7891234567890")
        + b"EAN-13 11 digits\n" + gs_k_a(2, b"78912345678")
        + b"EAN-13 letter\n" + gs_k_b(67, b"78912345678X")
        + b"UPC-A 10 digits\n" + gs_k_a(0, b"0360002914")
        + b"UPC-E NSC 1\n" + gs_k_a(1, b"11234565")
        + b"UPC-E not compressible\n" + gs_k_a(1, b"01234567890")
        + b"EAN-8 9 digits\n" + gs_k_b(68, b"963850741")
        + b"CODE39 lowercase\n" + gs_k_a(4, b"abc")
        + b"CODE39 star inside\n" + gs_k_a(4, b"AB*CD")
        + b"ITF odd count, function B\n" + gs_k_b(70, b"12345")
        + b"ITF odd count, function A (last digit dropped)\n" + gs_k_a(5, b"12345")
        + b"CODABAR without start/stop\n" + gs_k_a(6, b"40156")
        + b"CODE93 byte 0x80\n" + gs_k_b(72, b"AB\x80")
        + b"CODE128 without code set\n" + gs_k_b(73, b"ABC")
        + b"CODE128 C value 100\n" + gs_k_b(73, b"{C\x64")
        + b"GS1-128 (not drawn)\n" + gs_k_b(74, b"{A0123")
        + b"GS1 DataBar (not drawn)\n" + gs_k_b(75, b"2001234567890")
        + b"unknown m=10\n" + GS + b"k\x0a" + b"text after m\n"
        + b"function B n=0\n" + gs_k_b(73, b"")
        + b"function A missing NUL\n" + GS + b"k\x04" + b"A" * 260 + b"\n"
    )


def barcode_settings():
    # HAND-WRITTEN: GS h / GS w / GS H / GS f ranges and ESC @ defaults.
    return (
        GS + b"h\x00" + GS + b"w\x01" + GS + b"w\x07" + GS + b"H\x04" + GS + b"f\x05"
        + b"defaults: 162 dots, module 3, no HRI\n" + gs_k_a(3, b"9638507")
        + GS + b"h\x20" + GS + b"w\x06" + GS + b"H\x33" + GS + b"f\x31"
        + b"32 dots, module 6, HRI both in font B ('3', '1')\n" + gs_k_a(3, b"9638507")
        + GS + b"f\x02" + b"font C ignored\n" + gs_k_a(3, b"9638507")
        + ESC + b"@" + b"after ESC @\n" + gs_k_a(3, b"9638507")
        + b"too wide: ignored\n" + GS + b"w\x06" + gs_k_b(73, b"{B" + b"W" * 20)
    )


def qr_edge_cases():
    # HAND-WRITTEN: print without data, data too long, invalid parameters,
    # a symbol wider than the paper, store with m != 48, mid-line print.
    return (
        b"print without data\n" + qr_fn(81, b"0")
        + b"store with m=49 (ignored)\n" + qr_fn(80, b"1abc") + qr_fn(81, b"0")
        + b"module 0 and 17 ignored\n" + qr_fn(67, b"\x00") + qr_fn(67, b"\x11")
        + b"level 52 ignored, model 52 ignored\n" + qr_fn(69, b"4") + qr_fn(65, b"4\x00")
        + b"wrong length for fn 67\n" + qr_fn(67, b"\x04\x04")
        + b"unknown fn 70\n" + qr_fn(70, b"0")
        + b"too long for level H\n" + qr_fn(69, b"3") + qr_fn(80, b"0" + b"x" * 1300)
        + qr_fn(81, b"0")
        + b"too wide (module 16)\n" + qr_fn(69, b"0") + qr_fn(67, b"\x10")
        + qr_fn(80, b"0" + b"y" * 100) + qr_fn(81, b"0")
        + b"mid-line " + qr_fn(67, b"\x03") + qr_fn(81, b"0") + b"ignored\n"
        + b"size info (no reply)\n" + qr_fn(82, b"0")
        + b"stored data survives a print\n" + qr_fn(81, b"0")
        + b"ESC @ clears it\n" + ESC + b"@" + qr_fn(81, b"0")
        + b"end\n"
    )


def unsupported_symbols():
    # HAND-WRITTEN: other GS ( k symbols, GS 8 L, GS * / GS /, FS p and an
    # unimplemented GS ( E: consumed by their declared length.
    pdf = b"0P0" + b"PDF417 data"
    return (
        b"PDF417\n" + GS + b"(k" + bytes([len(pdf), 0]) + pdf
        + GS + b"(k\x03\x000Q0"
        + b"MaxiCode\n" + GS + b"(k\x04\x002A2\x00"
        + b"DataMatrix\n" + GS + b"(k\x05\x006P0AB"
        + b"GS 8 L\n" + GS + b"8L\x06\x00\x00\x0002\x01\x02\x03\x04"
        + b"GS * and GS /\n" + GS + b"*\x01\x01" + b"\xff" * 8 + GS + b"/\x00"
        + b"FS p\n" + FS + b"p\x01\x00"
        + b"GS ( E\n" + GS + b"(E\x03\x00\x01\x02\x03"
        + b"ESC * m=2 (header only)\n" + ESC + b"*\x02\x05\x00"
        + b"end\n"
    )


def incomplete_qr_eof():
    # HAND-WRITTEN: stream ends inside GS ( k function 180.
    return b"line\n" + GS + b"(k\x10\x001P0abc"


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
    # HAND-WRITTEN: stage 3-5 commands, all decoded since stage 5 (the name
    # predates it): cut, drawer, raster, QR, barcode, DLE EOT, GS r, GS a.
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


# --------------------------------------------------------------------------
# Stage 5: status (python-escpos helpers + HAND-WRITTEN requests)
# --------------------------------------------------------------------------

# Every state the status fixtures are run in: normal (no tag), each state
# alone, cover open + paper out, and everything at once.
STATES = [
    "",
    ".drawerOpen",
    ".paperOut",
    ".coverOpen",
    ".offline",
    ".coverOpen+paperOut",
    ".coverOpen+drawerOpen+offline+paperOut",
]


def status_requests():
    # python-escpos: query_status / is_online (DLE EOT 1), paper_status
    # (DLE EOT 4). HAND-WRITTEN: the other requests of the reference.
    p = StatusDummy()
    p.hw("INIT")
    p.text("printed only when online\n")
    p.query_status(RT_STATUS_ONLINE)
    p.is_online()
    p.paper_status()
    p.query_status(RT_STATUS_PAPER)
    p._raw(DLE + b"\x04\x02")  # DLE EOT 2: offline cause
    p._raw(DLE + b"\x04\x03")  # DLE EOT 3: error cause
    p._raw(GS + b"r\x01" + GS + b"r1")  # GS r 1 / 49: paper sensor
    p._raw(GS + b"r\x02" + GS + b"r2")  # GS r 2 / 50: drawer connector
    p._raw(DLE + b"\x14\x07\x01")  # DLE DC4 7 1: basic ASB status
    p.text("still offline?\n")
    p.cut()
    return p.output


def status_asb():
    # HAND-WRITTEN: GS a masks; replies at enable time and on the drawer
    # change caused by the connection's own kicks.
    return (
        GS + b"a\x02"  # online/offline: ASB sent at once
        + GS + b"a\x00"  # disabled: nothing
        + GS + b"a\x30"  # reserved bits only: disabled, nothing
        + GS + b"a\xff"  # every item: ASB sent at once
        + ESC + b"@"  # ESC @ disables ASB
        + ESC + b"p\x00\x19\xfa"  # kick with ASB disabled: no ASB
        + GS + b"a\x01"  # drawer item: ASB sent at once
        + DLE + b"\x14\x01\x00\x01"  # DLE DC4 1 kick: drawer already open
        + b"text\n"
    )


def status_asb_kick():
    # HAND-WRITTEN: drawer item enabled before the first kick, by ESC p
    # (buffered: discarded offline) and by DLE DC4 1 (real time).
    return (
        GS + b"a\x01"
        + ESC + b"p\x01\x32\x32"
        + DLE + b"\x14\x01\x01\x02"
        + GS + b"r\x02"
    )


def status_inside_data():
    # HAND-WRITTEN: status request bytes inside the data of other commands
    # are data (framed by length), never requests; one real request after.
    request = DLE + b"\x04\x01" + GS + b"r\x01" + DLE + b"\x14\x08\x01\x03\x14\x01\x06\x02\x08"
    assert len(request) % 2 == 0
    return (
        raster(0, 2, len(request) // 2, request)  # GS v 0 with requests as data
        + qr_fn(80, b"0" + DLE + b"\x04\x01" + DLE + b"\x04\x04")
        + qr_fn(81, b"0")
        + gs_k_b(73, b"{B" + b"\x10\x04\x01")  # CODE128 data (invalid char)
        + ESC + b"3" + DLE + b"\x04\x01"  # ESC 3 takes DLE as n; EOT, SOH stray
        + b"\n"
        + DLE + b"\x04\x01"  # the only request
    )


def status_realtime_functions():
    # HAND-WRITTEN: DLE DC4 functions and DLE ENQ.
    return (
        DLE + b"\x14\x01\x00\x01"  # pulse pin 2, 100 ms
        + DLE + b"\x14\x01\x01\x08"  # pulse pin 5, 800 ms
        + DLE + b"\x14\x01\x02\x01"  # m out of range: no pulse
        + DLE + b"\x14\x01\x00\x00"  # t = 0: no pulse
        + DLE + b"\x14\x01\x00\x09"  # t = 9: no pulse
        + DLE + b"\x14\x02\x01\x08"  # power-off: logged only
        + DLE + b"\x14\x02\x00\x00"  # power-off, invalid
        + DLE + b"\x14\x03\x00\x00\x00\x01\x00"  # buzzer: logged only
        + DLE + b"\x14\x07\x01"  # basic ASB status: answered
        + DLE + b"\x14\x07\x02" + DLE + b"\x14\x07\x04" + DLE + b"\x14\x07\x05"
        + b"cleared"
        + DLE + b"\x14\x08\x01\x03\x14\x01\x06\x02\x08"  # clear buffers
        + b"kept\n"
        + b"also kept"
        + DLE + b"\x14\x08\x01\x03\x14\x01\x06\x02\x09"  # invalid: ignored
        + b"\n"
        + DLE + b"\x14\x05"  # fn 5: not in the reference
        + DLE + b"\x05\x00" + DLE + b"\x05\x01" + DLE + b"\x05\x02"  # DLE ENQ
        + DLE + b"\x05\x03"  # DLE ENQ out of range
    )


def status_invalid():
    # HAND-WRITTEN: requests outside the reference, and the statuses this
    # printer does not have (ink, peeler, interface, DM-D): not answered.
    return (
        DLE + b"\x04\x00" + DLE + b"\x04\x05" + DLE + b"\x04\x31"
        + DLE + b"\x04\x07\x01" + DLE + b"\x04\x07\x02" + DLE + b"\x04\x07\x03"
        + DLE + b"\x04\x08\x03" + DLE + b"\x04\x08\x01"
        + DLE + b"\x04\x12\x01" + DLE + b"\x04\x12\x02"
        + GS + b"r\x00" + GS + b"r\x03" + GS + b"r\x04" + GS + b"r4" + GS + b"r\xff"
        + DLE + b"A"  # DLE + something else: unknown
        + b"ok\n"
    )


def print_suppressed():
    # python-escpos: a full receipt (text, styles, image, QR, barcode, cut,
    # drawer kick) sent in an error state: logged, nothing printed.
    p = printer()
    p.set(align="center", bold=True)
    p.text("MERCADO BOA VISTA\n")
    p.set(align="left", bold=False)
    p.image(checker(32, 16), impl="bitImageRaster")
    p.text("Arroz 5 kg             R$ 27,90\n")
    p.qr("PhantomPOS", native=True, size=3)
    p.barcode("7891234567895", "EAN13", function_type="A")
    p.cut()
    p.cashdraw(2)
    return p.output


def incomplete_status_eof():
    # HAND-WRITTEN: stream ends inside DLE EOT 7 (waiting for a).
    return b"line\n" + DLE + b"\x04\x07"


def incomplete_dc4_eof():
    # HAND-WRITTEN: stream ends inside DLE DC4 8.
    return b"line\n" + DLE + b"\x14\x08\x01\x03"


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
    "image_raster_logo": image_raster_logo,
    "image_raster_logo.58mm": image_raster_logo,
    "image_alignment": image_alignment,
    "image_wider_than_paper": image_wider_than_paper,
    "image_wider_than_paper.58mm": image_wider_than_paper,
    "qr_codes": qr_codes,
    "qr_models": qr_models,
    "barcode_ean13": barcode_ean13,
    "barcode_symbologies_a": lambda: barcode_symbologies("A"),
    "barcode_symbologies_b": lambda: barcode_symbologies("B"),
    "barcode_hri": barcode_hri,
    "receipt_codes": receipt_codes,
    "receipt_codes.58mm": receipt_codes,
    "unsupported_graphics": unsupported_graphics,
    "image_edge_cases": image_edge_cases,
    "image_truncated_eof": image_truncated_eof,
    "image_huge_declared": image_huge_declared,
    "barcode_invalid": barcode_invalid,
    "barcode_settings": barcode_settings,
    "qr_edge_cases": qr_edge_cases,
    "unsupported_symbols": unsupported_symbols,
    "incomplete_qr_eof": incomplete_qr_eof,
    **{f"status_requests{state}": status_requests for state in STATES},
    "status_asb": status_asb,
    "status_asb.paperOut": status_asb,
    "status_asb_kick": status_asb_kick,
    "status_asb_kick.offline": status_asb_kick,
    "status_inside_data": status_inside_data,
    "status_realtime_functions": status_realtime_functions,
    "status_realtime_functions.coverOpen": status_realtime_functions,
    "status_invalid": status_invalid,
    "print_suppressed.paperOut": print_suppressed,
    "print_suppressed.coverOpen": print_suppressed,
    "print_suppressed.offline": print_suppressed,
    "incomplete_status_eof": incomplete_status_eof,
    "incomplete_dc4_eof": incomplete_dc4_eof,
}


def main():
    for name, build in FIXTURES.items():
        write(name, build())
    print(f"wrote {len(FIXTURES)} fixtures to {OUT}")


if __name__ == "__main__":
    main()
