// A lazy screen's text loads with its code: `lazy(withText(["lane"], () =>
// import("./components/LaneView")))`. Both start together on first use, and
// the screen renders once both are in, so a first visit waits once and
// never shows a blank frame between its code and its words. Nothing loads
// before the screen is asked for.
import { getLocale, loadAll } from "./runtime";
import type { Ns } from "./types";

export function withText<T>(namespaces: readonly Ns[], load: () => Promise<T>): () => Promise<T> {
  return () => {
    // Text that won't load doesn't stop the screen: its useT asks again
    // (and shows the error boundary if that fails too).
    const text = loadAll(getLocale(), namespaces).catch(() => {});
    return Promise.all([load(), text]).then(([m]) => m);
  };
}
