import { defineConfig } from "vite";

// In development the gateway serves content and the display-protocol WebSocket; Vite serves the page.
const gateway = process.env.VIEWER_GATEWAY ?? "http://127.0.0.1:8765";

export default defineConfig({
  server: {
    proxy: {
      "/content": gateway,
      "/viewer": { target: gateway.replace(/^http/, "ws"), ws: true },
    },
  },
  build: {
    target: "es2022",
    outDir: "dist",
  },
});
