import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { locales } from "./tools/vite-locales.mjs";

// Tauri expects a fixed dev port and must not have Vite clear its output.
// locales(): the translation files served and shipped as they are, at /locales/<language>/<ns>.json,
// so the window fetches only the language it shows (Drover's plugin, mu-146).
export default defineConfig({
  plugins: [react(), locales()],
  clearScreen: false,
  // fs.allow: the app's own folder, plus the manual's pages (docs/manual), bundled with ?raw.
  server: { port: 1450, strictPort: true, host: "127.0.0.1", watch: { ignored: ["**/src-tauri/**"] }, fs: { allow: [".", "../docs/manual"] } },
  build: { target: "safari16" },
  // css: the card-look tests read styles.css (?raw) to check it uses what they test (0026), and
  // fonts.test.ts reads fonts.css (mu-150).
  // setupFiles: tests read the translation files from disk, with English loaded (src/i18n/testSetup.ts).
  test: { environment: "node", include: ["src/**/*.test.{ts,tsx}"], setupFiles: ["src/i18n/testSetup.ts"], css: { include: [/styles\.css/, /fonts\.css/] } },
});
