# Extension runtime spike

Throwaway code that measured the JS engine and WASI 0.3 async for decision
0060. It is not product code; no crate depends on it, and the main workspace
does not build it.

- `wit/spike.wit`: the small world both engines implement.
- `js-boa/`, `js-quickjs/`: the Boa and QuickJS-NG guests (`wasm32-wasip2`).
  QuickJS-NG needs wasi-sdk; point the `WASI_SDK` variable at it.
- `host/`: loads a component, applies the memory cap and epoch deadline, and
  runs the six cases. Usage: `spike-host <component.wasm>`.
- `async-probe/`: checks that Wasmtime 51 builds an engine with
  component-model async on and that `wasmtime_wasi::p3` resolves.

Each directory with its own `[workspace]` builds on its own:

```
cargo build --release --target wasm32-wasip2 -p js-boa
cargo build --release -p spike-host
```
