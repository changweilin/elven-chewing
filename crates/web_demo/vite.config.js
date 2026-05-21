import os from "node:os";
import { defineConfig } from "vite";

const machineName = os.hostname();
const extraAllowedHosts = (process.env.VITE_ALLOWED_HOSTS ?? "")
  .split(",")
  .map((host) => host.trim())
  .filter(Boolean);

const allowedHosts = [
  ...new Set([
    "localhost",
    ".localhost",
    ".local",
    ".lan",
    ".home.arpa",
    ".ts.net",
    machineName,
    machineName.toLowerCase(),
    `${machineName}.local`,
    `${machineName.toLowerCase()}.local`,
    ...extraAllowedHosts,
  ].filter(Boolean)),
];

const lanServer = {
  // Bind every interface so LAN and Tailscale peers can reach the server.
  // `host: true` is shorthand for 0.0.0.0 + ::.
  host: true,
  // Vite allows IP literals by default. These additions cover Windows host
  // names, mDNS/router LAN names, Tailscale MagicDNS, and explicit overrides.
  allowedHosts,
};

// Serve `static/` (with the hand-written index.html + wasm-pack output)
// directly so we don't fight bundler-side asset rewriting.
export default defineConfig({
  root: "static",
  // GitHub Pages serves project sites from /<repo>/; relative asset URLs keep
  // the generated bundle portable under that subpath and local previews.
  base: "./",
  server: {
    ...lanServer,
    port: 5173,
    strictPort: true,
  },
  preview: {
    ...lanServer,
    port: 4173,
    strictPort: true,
  },
  build: {
    target: "es2022",
    outDir: "../dist",
    emptyOutDir: true,
  },
  // `.wasm` is treated as an asset URL by wasm-bindgen `--target web`;
  // Vite serves it with the right MIME type out of the box.
});
