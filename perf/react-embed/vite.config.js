// SPDX-License-Identifier: MIT
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const PKG = join(ROOT, "packages/nitro-xml");

export default defineConfig({
  plugins: [react({ jsxRuntime: "automatic" })],
  root: dirname(fileURLToPath(import.meta.url)),
  resolve: {
    alias: {
      "nitro-xml/loader": join(PKG, "loader/index.js"),
      "nitro-xml": PKG,
    },
  },
  esbuild: {
    jsx: "automatic",
  },
  server: {
    port: 5179,
    strictPort: true,
    fs: {
      allow: [ROOT],
    },
  },
  preview: {
    port: 5179,
    strictPort: true,
  },
  optimizeDeps: {
    exclude: ["nitro-xml", "nitro-xml/loader"],
  },
  build: {
    target: "esnext",
    sourcemap: true,
  },
});
