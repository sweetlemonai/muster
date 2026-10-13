import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must not have Vite clear its output.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // fs.allow: the app's own folder, plus the manual's pages (docs/manual), bundled with ?raw.
  server: { port: 1450, strictPort: true, host: "127.0.0.1", watch: { ignored: ["**/src-tauri/**"] }, fs: { allow: [".", "../docs/manual"] } },
  build: { target: "safari16" },
  // css: the card-look tests read styles.css (?raw) to check it uses what they test (0026).
  test: { environment: "node", include: ["src/**/*.test.ts"], css: { include: [/styles\.css/] } },
});
