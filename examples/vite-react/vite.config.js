// SPDX-License-Identifier: MIT
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "../..");
const PKG = join(ROOT, "packages/nitro-xml");
const REACT = join(ROOT, "packages/nitro-xml-react");

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      { find: "@nitro-xml/react", replacement: join(REACT, "src/index.ts") },
      { find: "nitro-xml/loader", replacement: join(PKG, "loader/index.js") },
      { find: "nitro-xml/wasm", replacement: join(PKG, "pkg/nitro_xml_wasm.js") },
      { find: "nitro-xml", replacement: join(PKG, "index.js") },
    ],
  },
  server: {
    host: "127.0.0.1",
    port: 5180,
    strictPort: true,
    fs: { allow: [ROOT] },
  },
  optimizeDeps: {
    exclude: ["nitro-xml", "nitro-xml/loader", "@nitro-xml/react"],
  },
  assetsInclude: ["**/*.wasm"],
});
