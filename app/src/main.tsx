import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
import { apply, load } from "./themes";

// The chosen theme and accent before the first paint, and again when the Mac switches light/dark.
apply(load());
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => apply(load()));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
