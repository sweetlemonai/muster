import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
import "./fonts.css";
import { startLocale } from "./i18n";
import { apply, load } from "./themes";
import { installZoomKeys } from "./zoom";

// The chosen theme and accent before the first paint, and again when the Mac switches light/dark.
apply(load());
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => apply(load()));
// The remembered text size, and ⌘+ ⌘− ⌘0 (zoom.ts).
installZoomKeys();

// The last language's text (and <html lang dir>) before the first render, so nothing shows in
// English first; then the window follows the core's language (i18n/boot.ts, mu-146). The board's
// is the first screen's, and the waiting list and the task pane read its words too (mu-147).
void startLocale(["shell", "board"]).finally(() =>
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  ),
);
