# NexusLang Playground WASM

This directory contains the WebAssembly artifact loaded by
`../../nexuslang-playground.html`.

Build it from the workspace root:

```bash
./scripts/build-playground-wasm.sh
```

For browser validation, serve the repository root:

```bash
python3 -m http.server 8091 --bind 127.0.0.1
```

Then open:

```text
http://127.0.0.1:8091/nexuslang-playground.html
```

The playground should report `WASM pronto`, execute the default example, and
render parser/checker/runtime diagnostics with stage/message/line/column when
the input is invalid.
