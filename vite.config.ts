import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  // Polling keeps the preview current when filesystem notifications are unavailable.
  server: { port: 1420, strictPort: true, watch: { usePolling: true, interval: 500 } },
  clearScreen: false,
  build: { target: "safari16" },
});
