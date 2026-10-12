import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must not have Vite clear its output.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1450, strictPort: true, host: "127.0.0.1", watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "safari16" },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
