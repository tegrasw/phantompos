// DEVELOPMENT ONLY. A mock backend for working on the UI in a plain browser
// (`pnpm dev`, then open http://localhost:1420/?demo). App.svelte imports this
// module only behind `import.meta.env.DEV`, so production builds never include
// it.
//
// Query parameters:
//   ?demo            listening; plays a sample receipt shortly after start
//   ?demo=empty      listening, no traffic
//   ?demo=error      listener error (port in use)
//   ?demo=stopped    listener stopped
//   ?demo=starting   never answers get_state
//   ?demo=burst      listening; floods ~2,000 commands from three connections
//   &devpanel=0      hide the floating demo controls (for screenshots)
//
// The same actions are scriptable from the console via `window.phantomDemo`.

import type {
  BackendConnector,
  CommandBody,
  ConnectionsStatus,
  DecodedCommand,
  ListenerState,
  ListenerStatus,
} from "../backend";

// Code page 860 (Portuguese) for the accented characters the demo uses.
const CP860: Record<string, number> = {
  Ç: 0x80, ü: 0x81, é: 0x82, â: 0x83, ã: 0x84, à: 0x85, Á: 0x86, ç: 0x87,
  ê: 0x88, Ê: 0x89, è: 0x8a, Í: 0x8b, Ô: 0x8c, ì: 0x8d, Ã: 0x8e, Â: 0x8f,
  É: 0x90, À: 0x91, È: 0x92, ô: 0x93, õ: 0x94, ò: 0x95, Ú: 0x96, ù: 0x97,
  Ó: 0x9f, á: 0xa0, í: 0xa1, ó: 0xa2, ú: 0xa3, ª: 0xa6, º: 0xa7,
};

function encode(text: string): number[] {
  return Array.from(text, (ch) => CP860[ch] ?? (ch.charCodeAt(0) < 0x80 ? ch.charCodeAt(0) : 0x3f));
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
}

function receipt(): DecodedCommand[] {
  const s = new Stream();
  const rule = "-".repeat(48);
  s.add([0x1b, 0x40], { type: "initialize" })
    .add([0x1b, 0x74, 0x03], { type: "setCodePage", page: "cp860" })
    .align("center")
    .size(2, 2)
    .bold(true)
    .line("PADARIA BOA VISTA")
    .size(1, 1)
    .bold(false)
    .line("Rua das Flores, 123 - Centro")
    .line("São Paulo - SP")
    .line("CNPJ 12.345.678/0001-90")
    .add([0x1b, 0x4a, 0x18], { type: "feedDots", dots: 24 })
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
    .add([0x1b, 0x2d, 0x01], { type: "setUnderline", mode: 1 })
    .line("Obrigado e volte sempre!")
    .add([0x1b, 0x2d, 0x00], { type: "setUnderline", mode: 0 })
    .add([0x1d, 0x42, 0x01], { type: "setReverse", enabled: true })
    .line("  CUPOM NÃO FISCAL  ")
    .add([0x1d, 0x42, 0x00], { type: "setReverse", enabled: false })
    .add([0x1b, 0x4d, 0x01], { type: "setFont", font: "b" })
    .line(
      "Tributos aproximados: R$ 3,45 federal, R$ 2,10 estadual. Fonte: IBPT. Documento emitido para fins de teste, sem valor fiscal.",
    )
    .add([0x1b, 0x4d, 0x00], { type: "setFont", font: "a" })
    .add([0x1b, 0x52, 0x0c], { type: "setInternationalCharset", n: 12 })
    .add([0x0d], { type: "carriageReturn" })
    .add([0x1b, 0x64, 0x04], { type: "feedLines", lines: 4 })
    .add([0x1d, 0x56, 0x42, 0x00], { type: "unknown", reason: "Unrecognised command" });
  return s.commands;
}

function unknownSamples(): DecodedCommand[] {
  const s = new Stream();
  s.add([0x1b, 0x1d, 0x61, 0x01], { type: "unknown", reason: "Unrecognised command" })
    .add([0x1d, 0x28, 0x4c, 0x06, 0x00, 0x30, 0x45, 0x20, 0x20, 0x01, 0x01], {
      type: "unknown",
      reason: "Unrecognised command",
    })
    .add([0x1d, 0x76, 0x30, 0x00, 0x06, 0x00, 0x08, 0x00, ...Array.from({ length: 48 }, (_, i) => (i * 37) & 0xff)], {
      type: "unknown",
      reason: "Unrecognised command",
    })
    .add([0x1b], { type: "unknown", reason: "Incomplete sequence at end of stream" })
    .add([0x1b, 0x45, 0x01], { type: "setEmphasisVariant", enabled: true, level: 2, label: "future" });
  return s.commands;
}

const delay = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

export const connectDemo: BackendConnector = (handlers) => {
  const params = new URLSearchParams(location.search);
  const scenario = params.get("demo") ?? "";
  let disposed = false;
  let nextConnection = 1;
  const peers = new Map<number, string>();

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

  /** Sends commands as several batches, like fragmented socket reads. */
  async function send(commands: DecodedCommand[], chunk = 12, gapMs = 45): Promise<void> {
    if (listener.state !== "listening") return;
    const { id, peer } = open();
    for (let i = 0; i < commands.length && !disposed; i += chunk) {
      handlers.onCommandsReceived({ connectionId: id, peer, commands: commands.slice(i, i + chunk) });
      await delay(gapMs);
    }
    await delay(400);
    close(id);
  }

  async function burst(): Promise<void> {
    const sample = receipt();
    await Promise.all(
      [0, 1, 2].map(async () => {
        const commands: DecodedCommand[] = [];
        let offset = 0;
        for (let r = 0; r < 14; r++)
          for (const c of sample) {
            commands.push({ ...c, offset });
            offset += c.bytes.length;
          }
        await send(commands, 120, 4);
      }),
    );
  }

  const api = {
    receipt: () => send(receipt()),
    unknown: () => send(unknownSamples(), 2, 120),
    burst,
    listening: () => setListener("listening"),
    error: () => setListener("error"),
    stopped: () => setListener("stopped"),
  };
  (window as unknown as { phantomDemo: typeof api }).phantomDemo = api;

  // Initial snapshot, as `get_state` would deliver it.
  if (scenario !== "starting") {
    void delay(250).then(() => {
      if (disposed) return;
      handlers.onInitialState({ listener, connections: connectionsStatus() });
      if (scenario === "") void delay(600).then(() => api.receipt()).then(() => api.unknown());
      if (scenario === "burst") void delay(300).then(burst);
    });
  }

  const panel = params.get("devpanel") === "0" ? null : mountPanel(api);

  return () => {
    disposed = true;
    panel?.remove();
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
