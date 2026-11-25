# MassOrca mobile

The Android app (milestone M5, decision 0080). React Native 0.88 with TypeScript, on top of the Rust client in
`crates/client-ffi`, bound with uniffi-bindgen-react-native (`ubrn`). Everything except the native build runs without
the native Rust library.

## Install

Node 22.18 or newer (the token generator runs `.ts` files directly), then:

```sh
cd mobile
npm ci
```

Use `npm ci`, not `npm install`: CI installs from `package-lock.json` and it must stay the source of the versions.

## Check

```sh
npm run typecheck   # tsc, strict
npm run lint        # ESLint (@react-native config)
npm run tokens:check
npm test            # Jest
node scripts/generate-terminal-asset.ts --check   # the terminal page, see below
```

CI runs these in the job "mobile (typecheck, lint, test)" when a change touches `mobile/`.

## Design tokens

`src/theme/tokens.generated.ts` is generated from `docs/design/tokens.json` (the token contract, decision 0022).
Never edit it by hand. After changing the JSON:

```sh
npm run tokens      # rewrites src/theme/tokens.generated.ts
```

`npm run tokens:check` and the Jest test `src/theme/tokens.test.ts` fail when the committed file differs.

## Terminal page

`assets/terminal/page.html` is the page the terminal WebView loads (screen M-TM-01). The generator
`scripts/generate-terminal-asset.ts` inlines xterm.js (`@xterm/xterm`), its CSS and `@xterm/addon-fit` into
`android/app/src/main/assets/terminal/index.html`, so the terminal works without a network. Never edit the generated
file. After changing the template or the packages, run `node scripts/generate-terminal-asset.ts`; the `--check` form
runs in CI and in the Jest test `src/terminal/terminal-asset.test.ts`.

## Layout

- `App.tsx`: the app root. It takes an optional `binding`, so tests pass a fake.
- `src/binding/`: the app's view of the Rust binding.
  - `types.ts`: the app-facing contract. `fake.ts` is the scripted in-memory fake for tests.
  - `index.ts`: `getBinding()`, the seam the app calls. It loads the native binding on first use.
  - `native.ts`: the binding over the generated client. `native-shapes.ts` reads the generated shapes and
    `native-errors.ts` turns every failure into `FfiException`.
  - `host-store.ts`, `base64.ts` and `NativeMassorcaKeystore.ts`: the Rust `HostStore` on Android, over the Keystore
    module.
  - `watch-iterator.ts`: turns a pull-based watch into an async iterator.
- `src/generated/`, `src/index.tsx`, `src/NativeMassorcaMobile.ts`: generated TypeScript, see below.
- `src/navigation/`: the compact navigation stack (M-SH-01).
- `src/terminal/`: the terminal's state (`terminal-session.ts`), its WebView bridge (`terminal-bridge.ts`) and the Ctrl
  key (`control-key.ts`).
- `src/screens/`: one file per screen, with the model it shows (`*-model.ts`) and its tests beside it. Copy lives in
  `src/copy/`.
- `android/`: the Android project. `app/src/main/java/dev/massorca/app/keystore/` holds the Keystore module.

## Native binding

`ubrn.config.yaml` says where the generator reads the crate and writes its output. Output is split in two:

- **Committed (TypeScript only):** `src/generated/` (the bindings), `src/index.tsx` (the entry that installs the
  library), `src/NativeMassorcaMobile.ts` (the codegen spec of that install call). The typecheck and Jest need them.
- **Written by the build and ignored:** `cpp/`, `ios/`, `MassorcaMobile.podspec`, `android/CMakeLists.txt`,
  `android/cpp-adapter.cpp`, `android/src/main/` (the Kotlin glue), `android/app/src/main/jniLibs/` (the Rust libraries).

Build the Rust library for both ABIs and write the output (needs the Rust toolchain of `rust-toolchain.toml`,
`cargo-ndk` 4.1.2 and the Android NDK in `ANDROID_NDK_HOME`):

```sh
npx ubrn build android --config ubrn.config.yaml --and-generate
```

To regenerate only the TypeScript, run from the repository root, after `cargo build -p massorca-client-ffi --locked`:

```sh
mobile/node_modules/.bin/ubrn generate jsi bindings --library target/debug/massorca_client_ffi.dll \
  --ts-dir mobile/src/generated --cpp-dir mobile/cpp/generated
```

Use the `.so` of the host for `--library` outside Windows. Keep the formatting on (no `--no-format`): the committed
TypeScript is formatted with the project's prettier, as the build formats it. The `android-apk` CI job regenerates
everything and fails when `src/` differs from the commit, so a change to the Rust API must come with its regenerated
TypeScript.

Host entries live in the app's private storage (`filesDir/hosts`). The device key is sealed with an AES-256-GCM key
that the Android Keystore holds. The Keystore calls are synchronous, because the Rust store makes them from inside a
synchronous callback.

## Kotlin bindings on the JVM

`android/bindings-test/` is a small Gradle project that needs no React Native and no `node_modules`. It runs the
Keystore module's plain classes (`app/src/main/java/dev/massorca/app/keystore/storage/`) under Robolectric, with a
software key where the phone has the Keystore, and the Kotlin API that `uniffi-bindgen` generates from the host's build of
`massorca-client-ffi`: it pairs with and connects to a real server, round-trips a sealed key, and expects `Storage` for a
wrong key. CI runs `scripts/ci/kotlin-bindings-test.sh` (needs `uniffi-bindgen` 0.31.2 and Gradle 8.14 on `PATH`).

## Not done yet

- **Owner changes to `package.json`** (not made here; the native build needs them):
  - `"repository"`: ubrn refuses to read a `package.json` without it.
  - `"codegenConfig"`: `{ "name": "MassorcaMobile", "type": "modules", "jsSrcsDir": "src", "android": { "javaPackageName": "dev.massorca.app.binding" } }`.
    The generated and hand-written specs are under `src/`.
  - `"@ubjs/core": "0.31.0-7"` as a dependency: the generated TypeScript imports its runtime.
- **Not verified:** the Android build. This machine has no Android SDK or NDK, so the Gradle, CMake and Kotlin wiring
  has only been checked by the TypeScript side. The `android-apk` job is the first real build.
- **Device name:** every phone calls itself "Android phone" to its hosts (`src/binding/index.ts`). A per-device name
  needs a choice from the owner.
- **Terminal entry point:** the binding has no terminal list yet (`terminal.list` exists in the client crate but
  `client-ffi` does not export it, and the agent sessions carry no terminal id). The terminal screen takes its terminal
  id from its route, so nothing opens it until the binding exposes that id.
- **Not checked on a device:** the terminal page runs in the WebView only under Jest (a double). Key sequences, the
  Ctrl key, resizing and the WebView's file URL need a run on a phone or an emulator.
- Emulator flows (A11) and the phone validation (A12) are not started.
