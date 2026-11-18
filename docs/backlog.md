# Backlog

Ideas that are out of scope for PhantomPOS v1. Nothing listed here is implemented or
planned for a specific release; an item only moves into scope through a change to the
specification.

## Out of scope for v1

- Web interface, HTTP server or WebSocket server.
- CLI, headless mode and Docker image.
- USB, serial and Bluetooth transports.
- Installation as a system printer (CUPS, Windows print spooler).
- Multiple virtual printers at the same time.
- Proprietary commands from Bematech, Elgin and Daruma.
- Receipt history preserved between runs.
- Automatic application updates.
- Drawing other graphics commands: `ESC *`, `GS ( L` / `GS 8 L`, `GS *` / `GS /`,
  `FS p` / `FS q` (today they are consumed and logged as unsupported).
- Other 2D symbols (PDF417, MaxiCode, DataMatrix, Aztec) and GS1-128, GS1 DataBar and
  CODE128 auto barcodes.
- True QR model 1, and QR scaling by character size (`GS !`).
- Incremental `receipt:updated` and a lighter `bytes` field for large commands in
  `commands:received`.
