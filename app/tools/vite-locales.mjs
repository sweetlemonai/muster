// Vite plugin: the translation files (src/i18n/locales/<language>/<ns>.json)
// served and shipped as they are, at /locales/<language>/<ns>.json, so the
// app fetches the one language it shows and no startup JavaScript lists
// them (Drover's task 0007 and decision 0021; mu-146 here). Each file is checked and minified on
// the way out; a broken one fails the build.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const DIR = "src/i18n/locales";
const NAME = /^[A-Za-z]+(-[A-Za-z]+)?$/;

function minified(file) {
  return JSON.stringify(JSON.parse(readFileSync(file, "utf8")));
}

/** @returns {import("vite").Plugin} */
export function locales() {
  return {
    name: "muster-locales",
    configureServer(server) {
      server.middlewares.use("/locales", (req, res, next) => {
        const m = /^\/([^/]+)\/([^/]+)\.json$/.exec(req.url ?? "");
        if (!m || !NAME.test(m[1]) || !NAME.test(m[2])) return next();
        const file = join(DIR, m[1], `${m[2]}.json`);
        let body;
        try {
          body = minified(file);
        } catch {
          return next();
        }
        res.setHeader("Content-Type", "application/json");
        res.end(body);
      });
    },
    generateBundle() {
      for (const lang of readdirSync(DIR)) {
        if (!NAME.test(lang) || !statSync(join(DIR, lang)).isDirectory()) continue;
        for (const f of readdirSync(join(DIR, lang))) {
          if (!f.endsWith(".json")) continue;
          this.emitFile({ type: "asset", fileName: `locales/${lang}/${f}`, source: minified(join(DIR, lang, f)) });
        }
      }
    },
  };
}
