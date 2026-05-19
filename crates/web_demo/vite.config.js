import { defineConfig } from "vite";

// Serve `static/` (with the hand-written index.html + wasm-pack output)
// directly so we don't fight bundler-side asset rewriting.
export default defineConfig({
  root: "static",
  server: {
    // Bind every interface so Tailscale peers can reach the dev server.
    // `host: true` is shorthand for 0.0.0.0 + ::.
    host: true,
    port: 5173,
    strictPort: true,
    // Vite blocks unknown Host headers by default to mitigate DNS rebinding.
    // `.ts.net` matches Tailscale magic-DNS hostnames like
    // `my-laptop.tailnet-XXXX.ts.net`. Adjust if you use a custom tailnet.
    allowedHosts: [".ts.net", "localhost"],
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  // `.wasm` is treated as an asset URL by wasm-bindgen `--target web`;
  // Vite serves it with the right MIME type out of the box.
});
