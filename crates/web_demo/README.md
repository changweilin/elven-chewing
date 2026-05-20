# Web demo

Browser sandbox for the chewing engine. Built with `wasm-bindgen` and served
as a static page; no server runtime required.

## Prerequisites

Once per machine:

```sh
cargo install wasm-pack
npm install        # installs vite into node_modules/
```

The bundle embeds the real libchewing dictionaries (`word.dat` / `tsi.dat` /
`symbols.dat`), so they must be present before building the wasm:

```sh
cargo xtask download   # unpacks them into build/installer/Dictionary/
```

`build.rs` stages those into `OUT_DIR`; if they're missing the wasm build fails
with a message pointing back here. Override the source dir with
`CHEWING_DICT_DIR` if you stage them elsewhere.

## Local dev (Tailscale-accessible)

```sh
npm run dev
```

What it does:

1. `wasm-pack build --target web --out-dir static/pkg --dev` — recompiles
   the wasm bundle.
2. `vite` — serves `static/` on `0.0.0.0:5173` so both `localhost:5173`
   and your Tailscale peer's `http://<machine>.tailnet-XXXX.ts.net:5173`
   resolve.

The `allowedHosts: [".ts.net", "localhost"]` entry in `vite.config.js`
keeps Vite's DNS-rebinding guard happy for magic-DNS hostnames. If your
tailnet uses a custom domain or you front the dev server with a different
hostname, append it there.

You'll likely also want to allow inbound 5173 in your firewall for the
Tailscale interface. On Windows:

```pwsh
New-NetFirewallRule -DisplayName "vite-dev tailscale" \
    -Direction Inbound -Action Allow -Protocol TCP -LocalPort 5173 \
    -InterfaceAlias "Tailscale"
```

## Production build

```sh
npm run build
```

Output lands in `crates/web_demo/dist/`. Static; upload anywhere. The
generated `index.html` references the wasm bundle with relative URLs so
it survives subpath hosting.

## Scope

This is engine-only. It exercises libchewing through the same
`chewing_engine_kit` adapter the headless `cargo test` job uses, so behaviour
stays in sync, and it embeds the real dictionaries so typing — including
multi-syllable phrase selection (e.g. 台灣 as one phrase) — matches desktop.
It does **not** simulate:

- TSF event flow (focus, composition, display attributes).
- Out-of-process IPC, AppContainer ACL, code signing.

For full integration coverage, the `windows-integration` CI job runs on a
self-hosted Windows runner — see `design/ci-self-hosted-runner.md`.

## Bundled dictionary

The browser has no filesystem, so the real `word.dat` / `tsi.dat` /
`symbols.dat` are compiled into the wasm: `build.rs` stages them from
`build/installer/Dictionary/` into `OUT_DIR`, `src/lib.rs` `include_bytes!`s
them, and `chewing_engine_kit::build_editor_embedded` builds the editor from
those in-memory Tries. This adds ~4.8 MB to the bundle. To slim it down, point
`CHEWING_DICT_DIR` at a trimmed `.dat` set built with `chewing-cli`.
