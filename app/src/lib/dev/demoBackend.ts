// DEVELOPMENT ONLY. A mock backend for working on the UI in a plain browser
// (`pnpm dev`, then open http://localhost:1420/?demo). App.svelte imports this
// module only behind `import.meta.env.DEV`, so production builds never include
// it.
//
// Query parameters:
//   ?demo            listening; prints a sequence of receipts live: 80 mm with
//                    a full cut and a drawer kick, 58 mm box drawing with a
//                    partial cut, then two connections printing at once
//                    (one ends without a cut)
//   ?demo=gallery    the same receipts printed quickly, a ruler test, then a
//                    receipt that stops half-way and stays "printing"
//   ?demo=cap        prints 205 small receipts (exercises the retention cap)
//   ?demo=empty      listening, no traffic
//   ?demo=error      listener error (port in use)
//   ?demo=stopped    listener stopped
//   ?demo=starting   never answers get_state
//   ?demo=burst      listening; floods ~2,000 commands from three connections
//   &paper=58mm      initial paper; &drawer=open  initial drawer state
//   &devpanel=0      hide the floating demo controls (for screenshots)
//
// The same actions are scriptable from the console via `window.phantomDemo`.

import type {
  BackendConnector,
  CommandBody,
  ConnectionsStatus,
  Cut,
  DecodedCommand,
  ListenerState,
  ListenerStatus,
  Paper,
} from "../backend";
import { DemoLayout } from "./demoLayout";

// Code page bytes for the characters the demo uses (CP860 accents, CP437
// box drawing, shades and symbols). Demo data only.
const BYTES: Record<string, number> = {
  Ç: 0x80, ü: 0x81, é: 0x82, â: 0x83, ã: 0x84, à: 0x85, Á: 0x86, ç: 0x87,
  ê: 0x88, Ê: 0x89, è: 0x8a, Í: 0x8b, Ô: 0x8c, ì: 0x8d, Ã: 0x8e, Â: 0x8f,
  É: 0x90, À: 0x91, È: 0x92, ô: 0x93, õ: 0x94, ò: 0x95, Ú: 0x96, ù: 0x97,
  Ó: 0x9f, á: 0xa0, í: 0xa1, ó: 0xa2, ú: 0xa3, ª: 0xa6, º: 0xa7,
  "─": 0xc4, "│": 0xb3, "┌": 0xda, "┐": 0xbf, "└": 0xc0, "┘": 0xd9, "├": 0xc3,
  "┤": 0xb4, "┬": 0xc2, "┴": 0xc1, "┼": 0xc5, "═": 0xcd, "║": 0xba, "╔": 0xc9,
  "╗": 0xbb, "╚": 0xc8, "╝": 0xbc, "╠": 0xcc, "╣": 0xb9, "╦": 0xcb, "╩": 0xca,
  "╬": 0xce, "╤": 0xd1, "╧": 0xcf, "╪": 0xd8, "╟": 0xc7, "╢": 0xb6, "╫": 0xd7,
  "╒": 0xd5, "╕": 0xb8, "╘": 0xd4, "╛": 0xbe, "╓": 0xd6, "╖": 0xb7, "╙": 0xd3,
  "╜": 0xbd, "░": 0xb0, "▒": 0xb1, "▓": 0xb2, "█": 0xdb, "▄": 0xdc, "▀": 0xdf,
  "▌": 0xdd, "▐": 0xde, "■": 0xfe, "·": 0xfa, α: 0xe0, ß: 0xe1, Γ: 0xe2, π: 0xe3,
  Σ: 0xe4, σ: 0xe5, µ: 0xe6, τ: 0xe7, Φ: 0xe8, Θ: 0xe9, Ω: 0xea, δ: 0xeb,
  "∞": 0xec, "±": 0xf1, "≥": 0xf2, "≤": 0xf3, "÷": 0xf6, "≈": 0xf7, "°": 0xf8,
  "√": 0xfb, "²": 0xfd, "⌠": 0xf4, "⌡": 0xf5, "ⁿ": 0xfc, "₧": 0x9e, "⌐": 0xa9,
};

function encode(text: string): number[] {
  return Array.from(text, (ch) => BYTES[ch] ?? (ch.charCodeAt(0) < 0x80 ? ch.charCodeAt(0) : 0x3f));
}

/** Builds a command stream with consistent offsets. */
class Stream {
  offset = 0;
  commands: DecodedCommand[] = [];

  add(bytes: number[], command: CommandBody): this {
    this.commands.push({ offset: this.offset, bytes, command });
    this.offset += bytes.length;
    return this;
  }
  init(): this {
    return this.add([0x1b, 0x40], { type: "initialize" });
  }
  text(value: string): this {
    return this.add(encode(value), { type: "text", text: value });
  }
  lf(): this {
    return this.add([0x0a], { type: "lineFeed" });
  }
  line(value: string): this {
    return this.text(value).lf();
  }
  columns(left: string, right: string, width = 48): this {
    return this.line(left + right.padStart(width - left.length));
  }
  bold(enabled: boolean): this {
    return this.add([0x1b, 0x45, enabled ? 1 : 0], { type: "setBold", enabled });
  }
  align(alignment: "left" | "center" | "right"): this {
    const n = { left: 0, center: 1, right: 2 }[alignment];
    return this.add([0x1b, 0x61, n], { type: "setAlignment", alignment });
  }
  size(width: number, height: number): this {
    return this.add([0x1d, 0x21, ((width - 1) << 4) | (height - 1)], {
      type: "setCharacterSize",
      width,
      height,
    });
  }
  font(font: "a" | "b"): this {
    return this.add([0x1b, 0x4d, font === "a" ? 0 : 1], { type: "setFont", font });
  }
  underline(mode: "off" | "oneDot" | "twoDots"): this {
    const n = { off: 0, oneDot: 1, twoDots: 2 }[mode];
    return this.add([0x1b, 0x2d, n], { type: "setUnderline", mode });
  }
  reverse(enabled: boolean): this {
    return this.add([0x1d, 0x42, enabled ? 1 : 0], { type: "setReverse", enabled });
  }
  codePage(page: number, codePage: string): this {
    return this.add([0x1b, 0x74, page], { type: "setCodePage", page, codePage });
  }
  feed(lines: number): this {
    return this.add([0x1b, 0x64, lines], { type: "feedLines", lines });
  }
  feedDots(dots: number): this {
    return this.add([0x1b, 0x4a, dots], { type: "feedDots", dots });
  }
  cut(mode: "full" | "partial"): this {
    return this.add([0x1d, 0x56, mode === "full" ? 0x41 : 0x42, 0x00], { type: "cut", mode, function: "b", feed: 0 });
  }
  kick(pin: 2 | 5 = 2): this {
    return this.add([0x1b, 0x70, pin === 2 ? 0 : 1, 0x32, 0x64], { type: "drawerKick", pin, onMs: 100, offMs: 200 });
  }
}

// ---------------------------------------------------------------------------
// Sample receipts
// ---------------------------------------------------------------------------

function bakery(s = new Stream(), cut: "full" | "partial" = "full"): Stream {
  const rule = "-".repeat(48);
  return s
    .init()
    .codePage(3, "cp860")
    .align("center")
    .size(2, 2)
    .bold(true)
    .line("PADARIA BOA VISTA")
    .size(1, 1)
    .bold(false)
    .line("Rua das Flores, 123 - Centro")
    .line("São Paulo - SP")
    .line("CNPJ 12.345.678/0001-90")
    .feedDots(24)
    .align("left")
    .line(rule)
    .bold(true)
    .columns("ITEM", "QTD  UNIT   TOTAL")
    .bold(false)
    .columns("Pão francês", "3  0,90    2,70")
    .columns("Café coado 300ml", "1  6,50    6,50")
    .columns("Pão de queijo", "4  3,25   13,00")
    .columns("Água mineral s/ gás", "2  3,00    6,00")
    .line(rule)
    .size(1, 2)
    .bold(true)
    .text("TOTAL\tR$ 28,20")
    .lf()
    .size(1, 1)
    .bold(false)
    .add([0x1b, 0x33, 0x1e], { type: "setLineSpacing", dots: 30 })
    .line("Pagamento: PIX")
    .add([0x1b, 0x32], { type: "resetLineSpacing" })
    .align("center")
    .underline("oneDot")
    .line("Obrigado e volte sempre!")
    .underline("off")
    .reverse(true)
    .line("  CUPOM NÃO FISCAL  ")
    .reverse(false)
    .font("b")
    .line(
      "Tributos aproximados: R$ 3,45 federal, R$ 2,10 estadual. Fonte: IBPT. Documento emitido para fins de teste, sem valor fiscal.",
    )
    .font("a")
    .add([0x1b, 0x52, 0x0c], { type: "setInternationalCharset", n: 12 })
    .add([0x0d], { type: "carriageReturn" })
    .feed(3)
    .cut(cut)
    .kick(2);
}

/** 58 mm (32 columns): box drawing, shades, double size, reverse, font B. */
function tableTicket(s = new Stream()): Stream {
  return s
    .init()
    .codePage(0, "cp437")
    // Line spacing = cell height, so vertical box lines join between rows
    // (as POS software does for boxes; the default 30 dots leaves gaps).
    .add([0x1b, 0x33, 0x18], { type: "setLineSpacing", dots: 24 })
    .align("center")
    .line("╔══════════════════════════════╗")
    .text("║")
    .bold(true)
    .text("   MESA 12 · COMANDA 0481     ")
    .bold(false)
    .line("║")
    .line("╚══════════════════════════════╝")
    .align("left")
    .line("┌──────────────┬─────┬─────────┐")
    .line("│ Item         │ Qtd │   Valor │")
    .line("├──────────────┼─────┼─────────┤")
    .line("│ Feijoada     │  2  │   89,80 │")
    .line("│ Caipirinha   │  3  │   44,70 │")
    .line("│ Pudim        │  1  │   17,90 │")
    .line("╞══════════════╪═════╪═════════╡")
    .line("│ Subtotal     │  6  │  152,40 │")
    .line("└──────────────┴─────┴─────────┘")
    .add([0x1b, 0x32], { type: "resetLineSpacing" })
    .line("░░░░▒▒▒▒▓▓▓▓████▓▓▓▓▒▒▒▒░░░░ ■ ■")
    .align("center")
    .size(2, 2)
    .bold(true)
    .line("R$ 152,40")
    .size(1, 1)
    .bold(false)
    .reverse(true)
    .line(" PAGO · CRÉDITO ")
    .reverse(false)
    .font("b")
    .line("Garçom: Antônio  ·  10% incluso")
    .line("Atenção: confira seu pedido")
    .font("a")
    .feed(2)
    .cut("partial");
}

/** 80 mm kitchen order: font B header, double width/height, underline. */
function kitchenOrder(s = new Stream()): Stream {
  return s
    .init()
    .codePage(3, "cp860")
    .font("b")
    .align("center")
    .line("COZINHA · IMPRESSO 14:02 · TERMINAL 03")
    .font("a")
    .size(3, 3)
    .bold(true)
    .line("#0482")
    .size(1, 1)
    .bold(false)
    .reverse(true)
    .size(2, 1)
    .line(" VIAGEM ")
    .size(1, 1)
    .reverse(false)
    .align("left")
    .feedDots(12)
    .size(1, 2)
    .line("2x X-Salada")
    .size(1, 1)
    .line("   - sem cebola, ponto médio")
    .size(1, 2)
    .line("1x Batata grande")
    .line("1x Açaí 500ml")
    .size(1, 1)
    .line("   + granola, banana")
    .underline("twoDots")
    .bold(true)
    .line("Observação: entregar às 14:30")
    .bold(false)
    .underline("off")
    .feed(2)
    .cut("partial");
}

/** 58 mm, ends when the connection closes (no cut): CP437 symbols. */
function noteSlip(s = new Stream()): Stream {
  return s
    .init()
    .codePage(0, "cp437")
    .bold(true)
    .line("CONFERÊNCIA DE CAIXA")
    .bold(false)
    .line("Operador: José · Caixa 02")
    .line("--------------------------------")
    .columns("Dinheiro", "R$ 412,00", 32)
    .columns("Cartão", "R$ 1.208,35", 32)
    .columns("PIX", "R$ 655,10", 32)
    .line("--------------------------------")
    .line("α ß Γ π Σ σ µ τ Φ Θ Ω δ ∞")
    .line("± ≥ ≤ ÷ ≈ ° √ ² ⁿ ⌠ ⌡ ⌐ ₧")
    .font("b")
    .line("Fechamento parcial, sem corte de papel.")
    .font("a");
}

/** Column rulers: 48 columns must fill 576 dots, 32 must fill 384. */
function ruler(columns: number, s = new Stream()): Stream {
  const digits = Array.from({ length: columns }, (_, i) => String((i + 1) % 10)).join("");
  const tens = Array.from({ length: columns }, (_, i) => ((i + 1) % 10 === 0 ? String(((i + 1) / 10) % 10) : "·")).join("");
  return s
    .init()
    .line(tens)
    .line(digits)
    .bold(true)
    .line(digits)
    .bold(false)
    .reverse(true)
    .line(digits)
    .reverse(false)
    .font("b")
    .line(Array.from({ length: Math.floor((columns * 12) / 9) }, (_, i) => String((i + 1) % 10)).join(""))
    .font("a")
    .size(2, 1)
    .line(digits.slice(0, columns / 2))
    .size(1, 1)
    .line("─".repeat(columns))
    .line("▒".repeat(columns))
    .cut("full");
}

function unknownSamples(): DecodedCommand[] {
  const s = new Stream();
  s.add([0x1b, 0x1d, 0x61, 0x01], { type: "unknown", reason: "Unrecognised command" })
    .add([0x1d, 0x28, 0x4c, 0x06, 0x00, 0x30, 0x45, 0x20, 0x20, 0x01, 0x01], {
      type: "unknown",
      reason: "Unrecognised command",
    })
    .add([0x1b], { type: "unknown", reason: "Incomplete sequence at end of stream" })
    .add([0x1b, 0x45, 0x01], { type: "setEmphasisVariant", enabled: true, level: 2, label: "future" });
  return s.commands;
}

/** Splits a stream into one batch per printed line (as a POS sends it). */
function byLine(commands: DecodedCommand[]): DecodedCommand[][] {
  const batches: DecodedCommand[][] = [];
  let current: DecodedCommand[] = [];
  for (const command of commands) {
    current.push(command);
    if (["lineFeed", "feedLines", "feedDots", "cut"].includes(command.command.type)) {
      batches.push(current);
      current = [];
    }
  }
  if (current.length) batches.push(current);
  return batches;
}

const delay = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

export const connectDemo: BackendConnector = (handlers) => {
  const params = new URLSearchParams(location.search);
  const scenario = params.get("demo") ?? "";
  let disposed = false;
  let nextConnection = 1;
  let nextReceipt = 1;
  const peers = new Map<number, string>();
  let paper: Paper = params.get("paper") === "58mm" ? "58mm" : "80mm";
  let drawerOpen = params.get("drawer") === "open";

  let listener: ListenerStatus = {
    state: scenario === "error" ? "error" : scenario === "stopped" ? "stopped" : "listening",
    address: "127.0.0.1",
    port: 9100,
    ...(scenario === "error" ? { message: "Port 9100 is already in use" } : {}),
  };

  const connectionsStatus = (): ConnectionsStatus => ({
    active: peers.size,
    peers: [...peers.values()],
  });

  function setListener(state: ListenerState): void {
    listener = {
      state,
      address: "127.0.0.1",
      port: 9100,
      ...(state === "error" ? { message: "Port 9100 is already in use" } : {}),
    };
    if (!disposed) handlers.onListenerStatus(listener);
  }

  function open(): { id: number; peer: string } {
    const id = nextConnection++;
    const peer = `127.0.0.1:${49152 + Math.floor(Math.random() * 16000)}`;
    peers.set(id, peer);
    if (!disposed) handlers.onConnectionsChanged(connectionsStatus());
    return { id, peer };
  }

  function close(id: number): void {
    peers.delete(id);
    if (!disposed) handlers.onConnectionsChanged(connectionsStatus());
  }

  interface PrintOptions {
    /** Paper for this job (simulates the user having switched it). */
    paper?: Paper;
    gapMs?: number;
    /** Stop after this many batches and keep the connection open. */
    holdAfter?: number;
  }

  /**
   * Sends a job like a POS would: one batch per line, the in-progress receipt
   * after every batch, a completed receipt on every cut and at the end.
   */
  async function print(commands: DecodedCommand[], options: PrintOptions = {}): Promise<void> {
    if (listener.state !== "listening") return;
    const { id, peer } = open();
    const paperFor = (): Paper => options.paper ?? paper;
    const layout = new DemoLayout(paperFor());
    const batches = byLine(commands);
    for (let i = 0; i < batches.length && !disposed; i++) {
      const batch = batches[i];
      handlers.onCommandsReceived({ connectionId: id, peer, commands: batch });
      let closed = false;
      for (const { command } of batch) {
        layout.apply(command);
        if (command.type === "cut") {
          const receipt = layout.close(command.mode as Cut, paperFor());
          if (receipt) handlers.onReceiptCompleted({ id: nextReceipt++, connectionId: id, peer, receipt });
          closed = true;
        } else if (command.type === "drawerKick") {
          drawerOpen = true;
          handlers.onDrawerKicked({ connectionId: id, peer, pin: command.pin === 5 ? 5 : 2, onMs: 100, offMs: 200 });
        }
      }
      if (!closed && !layout.isEmpty) handlers.onReceiptUpdated({ connectionId: id, peer, receipt: layout.snapshot() });
      if (options.holdAfter !== undefined && i + 1 >= options.holdAfter) return;
      await delay(options.gapMs ?? 70);
    }
    await delay(250);
    if (disposed) return;
    const tail = layout.close(null, paperFor());
    if (tail) handlers.onReceiptCompleted({ id: nextReceipt++, connectionId: id, peer, receipt: tail });
    close(id);
  }

  async function burst(): Promise<void> {
    const sample = bakery().commands;
    await Promise.all(
      [0, 1, 2].map(async () => {
        const commands: DecodedCommand[] = [];
        let offset = 0;
        for (let r = 0; r < 14; r++)
          for (const c of sample) {
            commands.push({ ...c, offset });
            offset += c.bytes.length;
          }
        await print(commands, { gapMs: 4 });
      }),
    );
  }

  async function sequence(gapMs: number): Promise<void> {
    await print(bakery().commands, { paper: "80mm", gapMs });
    await delay(gapMs * 4);
    await print(tableTicket().commands, { paper: "58mm", gapMs });
    await delay(gapMs * 4);
    await Promise.all([
      print(kitchenOrder().commands, { paper: "80mm", gapMs: gapMs * 1.3 }),
      delay(gapMs * 3).then(() => print(noteSlip().commands, { paper: "58mm", gapMs })),
    ]);
  }

  const api = {
    receipt: () => print(bakery().commands),
    boxes: () => print(tableTicket().commands),
    kitchen: () => print(kitchenOrder().commands),
    noCut: () => print(noteSlip().commands),
    parallel: () => Promise.all([print(kitchenOrder().commands), print(noteSlip().commands, { gapMs: 110 })]),
    ruler: () => print(ruler(paper === "58mm" ? 32 : 48).commands),
    slow: () => print(bakery().commands, { gapMs: 600 }),
    kick: async () => {
      const { id, peer } = open();
      drawerOpen = true;
      handlers.onDrawerKicked({ connectionId: id, peer, pin: 2, onMs: 100, offMs: 200 });
      await delay(200);
      close(id);
    },
    sequence: () => sequence(70),
    unknown: () => print(unknownSamples()),
    burst,
    listening: () => setListener("listening"),
    error: () => setListener("error"),
    stopped: () => setListener("stopped"),
  };
  (window as unknown as { phantomDemo: typeof api }).phantomDemo = api;

  // Initial snapshot, as `get_state` would deliver it.
  if (scenario !== "starting") {
    void delay(250).then(async () => {
      if (disposed) return;
      handlers.onInitialState({ listener, connections: connectionsStatus(), paper, drawerOpen });
      if (scenario === "") {
        await delay(600);
        await sequence(70);
      } else if (scenario === "gallery") {
        await delay(300);
        await print(bakery().commands, { paper: "80mm", gapMs: 5 });
        await print(tableTicket().commands, { paper: "58mm", gapMs: 5 });
        await print(ruler(48).commands, { paper: "80mm", gapMs: 5 });
        await print(ruler(32).commands, { paper: "58mm", gapMs: 5 });
        await print(noteSlip().commands, { paper: "58mm", gapMs: 5 });
        await print(kitchenOrder().commands, { paper: "80mm", gapMs: 5 });
        await print(bakery(new Stream(), "partial").commands, { paper: "80mm", gapMs: 60, holdAfter: 12 });
      } else if (scenario === "cap") {
        // Completed receipts only (no live phase), as fast as possible.
        const { id, peer } = open();
        for (let i = 0; i < 205 && !disposed; i++) {
          const layout = new DemoLayout("58mm");
          for (const { command } of ruler(32).commands) if (command.type !== "cut") layout.apply(command);
          const receipt = layout.close("full", "58mm");
          if (receipt) handlers.onReceiptCompleted({ id: nextReceipt++, connectionId: id, peer, receipt });
          if (i % 20 === 19) await delay(16);
        }
        close(id);
      } else if (scenario === "burst") {
        await delay(300);
        await burst();
      }
    });
  }

  const panel = params.get("devpanel") === "0" ? null : mountPanel(api);

  return {
    async setPaper(next: Paper): Promise<Paper> {
      await delay(120);
      if (next !== "58mm" && next !== "80mm") throw new Error(`invalid paper: ${String(next)}`);
      paper = next;
      return paper;
    },
    async closeDrawer(): Promise<void> {
      await delay(80);
      drawerOpen = false;
    },
    disconnect(): void {
      disposed = true;
      panel?.remove();
    },
  };
};

function mountPanel(actions: Record<string, () => unknown>): HTMLElement {
  const panel = document.createElement("div");
  panel.setAttribute("data-dev-demo", "");
  panel.style.cssText = [
    "position:fixed", "left:12px", "bottom:12px", "z-index:9999",
    "display:flex", "flex-wrap:wrap", "gap:4px", "align-items:center", "max-width:340px",
    "padding:6px 8px", "border:1px dashed #d946ef", "border-radius:6px",
    "background:#1a0b1f", "color:#f5d0fe", "font:11px/1.4 ui-monospace,monospace",
  ].join(";");
  const label = document.createElement("strong");
  label.textContent = "DEV DEMO";
  label.style.marginRight = "4px";
  panel.append(label);
  for (const [name, action] of Object.entries(actions)) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = name;
    button.style.cssText =
      "font:inherit;color:inherit;background:#2e1236;border:1px solid #6b2177;border-radius:4px;padding:1px 6px;cursor:pointer";
    button.addEventListener("click", () => void action());
    panel.append(button);
  }
  document.body.append(panel);
  return panel;
}
