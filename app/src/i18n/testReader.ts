// For tests only: the translation files read from disk (the app fetches
// them), null for a file a language doesn't have.
const files = import.meta.glob("./locales/*/*.json", { import: "default" });

export const fileNames = Object.keys(files);

export async function readFromDisk(locale: string, ns: string): Promise<unknown> {
  const file = files[`./locales/${locale}/${ns}.json`];
  return file ? file() : null;
}
