import type { Plugin } from "vite";

/** Serves and ships src/i18n/locales/<language>/<ns>.json at /locales/. */
export function locales(): Plugin;
