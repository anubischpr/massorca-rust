# 0060: Extension runtime spike: the JS engine and WASI 0.3 async

Status: accepted, 2026-11-22.

## Context

Decision 0001 and `docs/architecture/extensions.md` leave two spikes open
before the tier 2 host is built (§12, §13):

1. A JavaScript engine compiled as a WebAssembly component that loads source
   at runtime, so Creator mode can write extensions in JS without a compiler
   on the host. Its size, start time, memory and builds on Windows and Linux
   decide whether Creator mode writes JS or falls back to Rust (§11).
2. The state of WASI 0.3 async in the pinned Wasmtime, which decides whether
   agent adapters and jobs can use async exports or keep the streams and
   host-driven calls of §4.

The spike code is in `spikes/ext-runtime/`. It is not product code and no
crate depends on it.

## What was measured

One host (`spikes/ext-runtime/host`) on Wasmtime 51.0.0 loads each engine
component against the same small WIT world (`wit/spike.wit`: a `log` and a
`kv-get` host import, and a `run(source)` export). It applies the limits
from §4: a 128 MiB memory cap through a `ResourceLimiter`, and an epoch
deadline driven by a 10 ms ticker. It runs six cases: a trivial
expression, JSON over 2,000 items, 10,000 host calls, closures and
classes, an endless loop with a 1 s deadline, and allocation past the
memory cap.

Engines:

- **Boa 0.22**, pure Rust, built with `wasm32-wasip2` and wit-bindgen 0.63.
- **QuickJS-NG 0.16.2** through rquickjs 0.14, built with `wasm32-wasip2`
  and wasi-sdk 34.0 for the C part.

Both use the release profile `opt-level = "s"`, LTO, one codegen unit,
stripped, with panic set to abort.

| Measure | Boa (Windows) | Boa (Linux) | QuickJS-NG (Linux) |
|---|---|---|---|
| Component size | 4.64 MiB | 4.64 MiB | 0.76 MiB |
| Precompiled (`.cwasm`) | 12.36 MiB | 11.68 MiB | 2.42 MiB |
| Cold compile | 2.4 to 3.0 s | 6.0 s | 3.3 s |
| Load precompiled | 9 to 12 ms | 57.5 ms | 211 ms (noisy) |
| First instantiate | about 1 ms | 4.1 ms | 48.5 ms |
| Trivial run | 1.3 ms | 41.1 ms | 28.5 ms |
| JSON, 2,000 items | 273 to 423 ms | 506 ms | 190 ms |
| 10,000 host calls | 70 to 76 ms | 118 ms | 86 ms |
| Closures and classes | 406 to 539 ms | 971 ms | 195 ms |
| Endless loop, 1 s deadline | interrupted at 1.2 to 1.4 s | interrupted at 1.40 s | interrupted at 1.15 s |
| Peak memory, completed cases | | 2.2 to 8.8 MiB | 1.3 to 3.1 MiB |
| Allocation past 128 MiB | cap hit at 127.2 MiB, then an `unreachable` trap | the same | cap hit at 127.3 MiB, then a catchable `InternalError: out of memory` |

The four cases that completed returned the same values on both engines.
The Linux numbers come from a WSL distribution that other builds were
using at the same time. Treat them as relative, not absolute. The 211 ms
load for QuickJS-NG is one noisy sample, not a property of the engine.

WASI 0.3 async was checked against the published 51.0.0 sources and with a
probe (`spikes/ext-runtime/async-probe`). The probe builds an engine with
`wasm_component_model_async(true)` and resolves `wasmtime_wasi::p3`. It
compiled and ran on Windows.

- `component-model-async` is on by default in Wasmtime 51, and
  `wasmtime-wasi` 51 enables its `p3` module by default. The release notes
  for Wasmtime 46 say WASI 0.3 is on by default from that release.
- `Config::async_support` is deprecated and has no effect in 51.
- The Wasmtime stability page lists `component-model-async` as Tier 1 and
  does not list WASIp3.
- wit-bindgen 0.63 generates async bindings with stream and future readers.
  A guest uses them from the existing `wasm32-wasip2` target, which needs
  Rust 1.90 or newer; we pin 1.98.1. The `wasm32-wasip3` target itself first
  reaches stable in Rust 1.100. For a wasm32 target, wit-bindgen links its
  prebuilt C shim, so a Rust guest needs no C toolchain.
- Open issue bytecodealliance/wasmtime#11209 reports a `call_async`
  performance regression with `component-model-async` on. Issue #14286
  (`pollable.block` from WASI 0.2 blocks the whole store under
  `run_concurrent`) was closed in September 2026. Whether its fix is in
  51.0.0 was not checked.

## Decision

- **The bundled JS engine is QuickJS-NG**, through rquickjs, compiled as a
  `wasm32-wasip2` component. It is six times smaller than Boa, precompiles
  to a fifth of the size, ran the heavier cases two to five times faster,
  and turns running out of memory into a JS error that the extension can
  catch instead of a trap that kills the instance.
- **The component is built once, in CI, and shipped as a `.wasm` file.** A
  component does not depend on the OS, so every desktop and server uses the
  same file. Building it needs wasi-sdk (the `WASI_SDK` environment
  variable, not `WASI_SDK_PATH`); CI installs a pinned wasi-sdk on Linux.
  Users and developers never compile the engine. The host precompiles it to
  `.cwasm` for the local Wasmtime on first use and caches the result.
- **Boa is the fallback.** If QuickJS-NG stops building or a security issue
  in its C code cannot be fixed in time, Boa drops in behind the same WIT
  world. It costs size and speed but needs no C toolchain. The Rust fallback
  of §11 stays as the last resort and is not needed now.
- **World `massorca:extension@1.0.0` stays synchronous.** Agent adapters and
  jobs keep the streams and host-driven calls of §4. Wasmtime 51 can run
  async components, but this spike only compiled against the async API; no
  guest with async exports was run. One `call_async` issue is still open,
  and WASI 0.3 is not on the stability page. Async exports come later as a
  new, additive world version after a guest runs with them under the tier 2
  host's limits. Host functions may still be async on the host side; that
  does not change the world.
- **The limits of §4 hold.** The epoch deadline stopped an endless loop in
  both engines, and the `ResourceLimiter` held the 128 MiB cap.

## Not verified

- **The QuickJS-NG build on Windows.** It was built only on Linux. On
  Windows it needs the Windows wasi-sdk. rquickjs's `build.rs` calls
  `bin/clang` without `.exe`, and the SDK path must contain no spaces. This
  does not block anything, because the engine is built in CI on Linux and
  shipped as `.wasm`. It matters only for a developer who wants to rebuild
  the engine locally on Windows.
- **Deep recursion.** QuickJS-NG turns off its own stack check when built
  for WASI (`__wasi__`), so runaway recursion should end in a Wasm stack
  overflow trap rather than a JS `RangeError`. This was not run. The tier 2
  host must test it and treat the trap like any other crash (§4: three
  crashes in 10 minutes disable the extension).
- **QuickJS-NG on Windows at run time.** Only Boa was measured on Windows.
  QuickJS-NG was measured on Linux. The component is the same file, so the
  Windows host should behave the same, but this was not measured.
- **Async exports at run time**, as described above.

## Alternatives considered

- **Boa as the engine.** Pure Rust and simple to build anywhere, but 4.64 MiB
  per component, slower in every heavier case, and out of memory is a trap.
  Kept as the fallback.
- **Build the engine on each user's machine.** Rejected: it needs wasi-sdk
  and a Rust toolchain on every host for no gain, because the component does
  not depend on the OS.
- **Use async exports now.** Rejected for 1.0.0, for the reasons above. The
  world can add them later without breaking existing extensions.
- **Rust-only Creator mode (§11 fallback).** Not needed: the JS engine works
  within the limits.

## Consequences

- `docs/architecture/extensions.md` §4, §11 and §13 now point to this
  decision.
- The tier 2 host adds a CI job that builds the QuickJS-NG component with a
  pinned wasi-sdk and publishes the `.wasm` as a build artifact.
- The tier 2 host gets tests for deep recursion, out of memory and the epoch
  deadline against the shipped engine.
