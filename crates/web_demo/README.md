# Web demo

Browser sandbox for the chewing engine. Built with `wasm-bindgen` and served
as a static page; no server runtime required.

## Prerequisites

Once per machine:

```sh
cargo install wasm-pack
npm install        # installs vite into node_modules/
```

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
`chewing_engine_kit::build_editor` adapter the headless `cargo test` job uses,
so behaviour stays in sync. It does **not** simulate:

- TSF event flow (focus, composition, display attributes).
- Candidate-window rendering (the engine just exposes preedit/commit text).
- Out-of-process IPC, AppContainer ACL, code signing.

For full integration coverage, the `windows-integration` CI job runs on a
self-hosted Windows runner — see `design/ci-self-hosted-runner.md`.

## Replacing the bundled dictionary

The browser has no filesystem, so libchewing falls back to its built-in
`mini.dat` (a few-entry stub). To exercise real vocabulary, embed a real
`tsi.dat` / `word.dat` via `include_bytes!` and construct the editor with
`chewing::dictionary::Trie::new(&bytes[..])` instead of going through
`build_editor` — see `chewing::editor::Editor::new` for the manual path.
