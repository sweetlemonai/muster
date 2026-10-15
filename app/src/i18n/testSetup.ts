// Tests read the translation files from disk (the app fetches them), and
// start with all the English text loaded, as a window has its text before
// it renders: components render in English at once.
import { loadAll, setReader } from "./runtime";
import { fileNames, readFromDisk } from "./testReader";
import type { Ns } from "./types";

setReader(readFromDisk);

const namespaces = fileNames.filter((f) => f.startsWith("./locales/en/")).map((f) => f.replace(/^.*\/(.+)\.json$/, "$1") as Ns);
await loadAll("en", namespaces);
