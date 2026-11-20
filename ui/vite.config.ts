import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// The built UI is embedded into the twin-api Wasm component (components/twin-api/build.rs).
// `npm run dev` proxies /api to a running twin (TWIN_URL, default the VM's ingress).
export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@design": resolve(__dirname, "src/design-system") } },
  build: { outDir: "dist", assetsInlineLimit: 0, chunkSizeWarningLimit: 2000 },
  server: { proxy: { "/api": { target: process.env.TWIN_URL ?? "http://10.211.55.17:8088", changeOrigin: false } } },
});
