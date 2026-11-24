# MassOrca mobile

The Android app (milestone M5, decision 0080). React Native 0.88 with TypeScript, on top of the Rust client in
`crates/client-ffi`. This directory builds and tests without the native Rust library.

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
```

CI runs these four in the job "mobile (typecheck, lint, test)" when a change touches `mobile/`.

## Design tokens

`src/theme/tokens.generated.ts` is generated from `docs/design/tokens.json` (the token contract, decision 0022).
Never edit it by hand. After changing the JSON:

```sh
npm run tokens      # rewrites src/theme/tokens.generated.ts
```

`npm run tokens:check` and the Jest test `src/theme/tokens.test.ts` fail when the committed file differs.

## Layout

- `App.tsx`: the app root. It takes an optional `binding`, so tests pass a fake.
- `src/binding/`: the app's view of the Rust binding. `types.ts` mirrors the exported API, `fake.ts` is the scripted
  in-memory fake for tests, `index.ts` is the seam where the native binding plugs in, and `watch-iterator.ts` turns a
  pull-based watch into an async iterator.
- `src/navigation/`: the compact navigation stack (M-SH-01).
- `src/screens/`: screens. Most are placeholders that show their title until their task lands.

## Not done yet

The native binding is not wired: `src/binding/index.ts` throws `native binding not built`, so the app cannot start on a
device yet. Generating the binding with uniffi-bindgen-react-native, building the debug APK and the Android CI build are
the next task (A7, second half).
