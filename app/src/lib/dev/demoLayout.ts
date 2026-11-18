// DEVELOPMENT ONLY. A small TypeScript copy of core's receipt layout
// (core/src/receipt.rs) so the demo backend can produce realistic Receipt
// snapshots from the same commands it shows in the log. Demo data only: the
// real layout is core's, and this copy covers just the commands the demo
// sends.

import type {
  Alignment,
  CommandBody,
  Cut,
  ImageElement,
  LineElement,
  Paper,
  Receipt,
  Span,
  SpanStyle,
  UnderlineMode,
} from "../backend";
import { toBase64, type DemoBitmap } from "./demoImages";

/** What an image command prints (the demo has no core to decode its bytes). */
export interface DemoImage {
  kind: ImageElement["kind"];
  bitmap: DemoBitmap;
  /** Barcode human-readable text, printed below in font A. */
  hri?: string;
}

/** Image commands → their bitmap. Keyed by the command object. */
export const demoImages = new WeakMap<CommandBody, DemoImage>();

const FONTS = { a: { width: 12, height: 24, baseline: 21 }, b: { width: 9, height: 17, baseline: 16 } };
const DEFAULT_SPACING = 30;
const TAB_DOTS = 96;

interface State {
  font: "a" | "b";
  bold: boolean;
  underline: UnderlineMode;
  reverse: boolean;
  wm: number;
  hm: number;
  align: Alignment;
  spacing: number;
}

const initial = (): State => ({
  font: "a",
  bold: false,
  underline: "off",
  reverse: false,
  wm: 1,
  hm: 1,
  align: "left",
  spacing: DEFAULT_SPACING,
});

export class DemoLayout {
  #paper: Paper;
  #state = initial();
  #spans: Span[] = [];
  /** Span baselines while pending (core keeps them in `Span::y`). */
  #cursor = 0;
  #hasData = false;
  #elements: (LineElement | ImageElement)[] = [];
  #y = 0;

  constructor(paper: Paper) {
    this.#paper = paper;
  }

  get width(): number {
    return this.#paper === "58mm" ? 384 : 576;
  }

  get isEmpty(): boolean {
    return this.#elements.length === 0 && !this.#hasData;
  }

  apply(command: CommandBody): void {
    const s = this.#state;
    const image = demoImages.get(command);
    if (image) {
      this.#image(image);
      return;
    }
    switch (command.type) {
      case "text":
        for (const ch of String(command.text ?? "")) this.#put(ch);
        break;
      case "initialize":
        this.#spans = [];
        this.#cursor = 0;
        this.#hasData = false;
        this.#state = initial();
        break;
      case "lineFeed":
        this.#printAndFeed(s.spacing);
        break;
      case "horizontalTab":
        this.#cursor = Math.ceil((this.#cursor + 1) / TAB_DOTS) * TAB_DOTS;
        this.#hasData = true;
        break;
      case "feedLines":
        this.#printAndFeed(Number(command.lines) * s.spacing);
        break;
      case "feedDots":
        this.#printAndFeed(Number(command.dots));
        break;
      case "resetLineSpacing":
        s.spacing = DEFAULT_SPACING;
        break;
      case "setLineSpacing":
        s.spacing = Number(command.dots);
        break;
      case "setBold":
        s.bold = Boolean(command.enabled);
        break;
      case "setUnderline":
        s.underline = (command.mode as UnderlineMode) ?? "off";
        break;
      case "setReverse":
        s.reverse = Boolean(command.enabled);
        break;
      case "setFont":
        if (command.font === "a" || command.font === "b") s.font = command.font;
        break;
      case "setCharacterSize":
        s.wm = Number(command.width);
        s.hm = Number(command.height);
        break;
      case "setAlignment":
        if (!this.#hasData) s.align = command.alignment as Alignment;
        break;
    }
  }

  /** Prints the pending line and returns the receipt so far (null if empty). */
  close(cut: Cut, nextPaper: Paper): Receipt | null {
    if (this.#hasData) this.#printAndFeed(this.#state.spacing);
    const receipt = this.snapshot(cut);
    this.#elements = [];
    this.#y = 0;
    this.#paper = nextPaper;
    return receipt.elements.length > 0 ? receipt : null;
  }

  snapshot(cut: Cut = null): Receipt {
    return structuredClone({
      paper: this.#paper,
      width: this.width,
      height: this.#y,
      cut,
      elements: this.#elements,
    });
  }

  #style(): SpanStyle {
    const s = this.#state;
    return {
      font: s.font,
      bold: s.bold,
      underline: s.reverse ? "off" : s.underline,
      reverse: s.reverse,
      widthMultiplier: s.wm,
      heightMultiplier: s.hm,
    };
  }

  #put(ch: string): void {
    const style = this.#style();
    const metrics = FONTS[style.font];
    const charWidth = metrics.width * style.widthMultiplier;
    if (this.#hasData && this.#cursor + charWidth > this.width) this.#printAndFeed(this.#state.spacing);
    const last = this.#spans.at(-1);
    if (last && JSON.stringify(last.style) === JSON.stringify(style) && last.x + last.width === this.#cursor) {
      last.text += ch;
      last.columns++;
      last.width += charWidth;
    } else {
      this.#spans.push({
        text: ch,
        x: this.#cursor,
        y: metrics.baseline * style.heightMultiplier,
        width: charWidth,
        columns: 1,
        charWidth,
        charHeight: metrics.height * style.heightMultiplier,
        style,
      });
    }
    this.#cursor += charWidth;
    this.#hasData = true;
  }

  #printAndFeed(feed: number): void {
    const s = this.#state;
    if (!this.#hasData) {
      if (feed > 0) this.#push({ y: this.#y, height: 0, baseline: 0, advance: feed, alignment: s.align, contentWidth: 0, spans: [] });
      return;
    }
    const spans = this.#spans;
    const height = Math.max(...spans.map((sp) => sp.charHeight));
    const baseline = Math.max(...spans.map((sp) => sp.y));
    const contentWidth = Math.min(this.#cursor, this.width);
    const free = this.width - contentWidth;
    const offset = s.align === "center" ? Math.floor(free / 2) : s.align === "right" ? free : 0;
    for (const sp of spans) {
      sp.x += offset;
      sp.y = baseline - sp.y;
    }
    this.#push({ y: this.#y, height, baseline, advance: Math.max(feed, height), alignment: s.align, contentWidth, spans });
    this.#spans = [];
    this.#cursor = 0;
    this.#hasData = false;
  }

  #image({ kind, bitmap, hri }: DemoImage): void {
    if (this.#hasData) this.#printAndFeed(this.#state.spacing);
    const free = Math.max(0, this.width - bitmap.width);
    const align = this.#state.align;
    const x = align === "center" ? Math.floor(free / 2) : align === "right" ? free : 0;
    const { width, height } = bitmap;
    this.#elements.push({ type: "image", kind, x, y: this.#y, width, height, advance: height, data: toBase64(bitmap.bits) });
    this.#y += height;
    if (hri) {
      const saved = { ...this.#state };
      Object.assign(this.#state, { font: "a", bold: false, underline: "off", reverse: false, wm: 1, hm: 1 });
      for (const ch of hri) this.#put(ch);
      this.#printAndFeed(this.#state.spacing);
      this.#state = saved;
    }
  }

  #push(line: Omit<LineElement, "type">): void {
    this.#elements.push({ type: "line", ...line });
    this.#y += line.advance;
  }
}
