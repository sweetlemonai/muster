// The non-hook helpers: `tn` for text in click handlers, `tl` for the
// shared words, `sentences` for whole sentences one after another,
// `withText` to load a lazy screen's text with its code.
import { afterEach, describe, expect, it, vi } from "vitest";
import { PSEUDO } from "./locales";
import { sentences, tl, tn, unmarked } from "./lib";
import { FSI, PDI, pseudoText, setLocale } from "./runtime";

afterEach(async () => {
  await setLocale("en");
});

describe("tn", () => {
  it("reads a namespace's text in the language shown now", async () => {
    expect(tn("shell", "language.label")).toBe("Language");
    expect(tn("shell", "common:sentences", { a: "A.", b: "B." })).toBe("A. B.");
    await setLocale(PSEUDO);
    expect(tn("shell", "language.label")).toBe(pseudoText("Language"));
    await setLocale("fa");
    expect(tn("shell", "language.label")).toBe("زبان");
  });

  it("fills placeholders and plurals", () => {
    expect(tn("native", "tray.waiting", { count: 1 })).toBe("1 thing waits for you");
    expect(tn("native", "tray.waiting", { count: 3 })).toBe("3 things wait for you");
  });

  it("checks keys at compile time", () => {
    // @ts-expect-error: not a key in shell.json
    expect(() => tn("shell", "language.lable")).toBeTypeOf("function");
  });
});

describe("tl", () => {
  it("reads the shared namespace", async () => {
    expect(tl("sentences", { a: "A.", b: "B." })).toBe("A. B.");
    await setLocale("fa");
    // Right to left, each value is its own run; unmarked takes the marks off for data.
    expect(tl("sentences", { a: "A.", b: "B." })).toBe(`${FSI}A.${PDI} ${FSI}B.${PDI}`);
    expect(unmarked(tl("sentences", { a: "A.", b: "B." }))).toBe("A. B.");
  });
});

describe("sentences", () => {
  it("joins whole sentences through the language's joiner, skipping empty ones", async () => {
    expect(sentences(["One.", "", "Two.", "Three."])).toBe("One. Two. Three.");
    expect(sentences(["Only."])).toBe("Only.");
    expect(sentences([])).toBe("");
    // Pseudo brackets the joiner: each pair went through common:sentences.
    await setLocale(PSEUDO);
    const join = (a: string, b: string) => pseudoText("{{a}} {{b}}").replace("{{a}}", a).replace("{{b}}", b);
    expect(sentences(["One.", "Two.", "Three."])).toBe(join(join("One.", "Two."), "Three."));
  });
});

describe("withText", () => {
  it("loads nothing until the screen is asked for, then its text and code together", async () => {
    // A fresh copy of the layer with nothing loaded, as at startup.
    vi.resetModules();
    const runtime = await import("./runtime");
    const { withText } = await import("./prefetch");
    const asked: string[] = [];
    const files: Record<string, (v: unknown) => void> = {};
    runtime.setReader((l, ns) => {
      asked.push(`${l}/${ns}`);
      return new Promise((resolve) => (files[`${l}/${ns}`] = resolve));
    });
    let code: (m: { default: string }) => void = () => {};
    const importer = vi.fn(() => new Promise<{ default: string }>((resolve) => (code = resolve)));
    const screen = withText(["pane", "help"], importer);
    // Defining the lazy screen (startup) loads nothing.
    expect(asked).toEqual([]);
    expect(importer).not.toHaveBeenCalled();

    let done: { default: string } | undefined;
    void screen().then((m) => (done = m));
    // Both at once: the text doesn't wait for the code, nor the code for the text.
    expect(asked).toEqual(["en/pane", "en/help"]);
    expect(importer).toHaveBeenCalledOnce();

    code({ default: "TaskPanel" });
    await new Promise((r) => setTimeout(r, 0));
    // The code is in, the text isn't: not ready yet, so the screen never
    // renders and then waits again for its words.
    expect(done).toBeUndefined();
    files["en/pane"]({ a: "A" });
    files["en/help"]({ b: "B" });
    await vi.waitFor(() => expect(done).toEqual({ default: "TaskPanel" }));
    expect(runtime.isLoaded("en", "pane") && runtime.isLoaded("en", "help")).toBe(true);
  });

  it("still opens the screen when its text fails to load (useT tries again)", async () => {
    vi.resetModules();
    const runtime = await import("./runtime");
    const { withText } = await import("./prefetch");
    runtime.setReader(async () => {
      throw new Error("offline");
    });
    await expect(withText(["board"], async () => ({ default: "Board" }))()).resolves.toEqual({ default: "Board" });
    expect(runtime.isLoaded("en", "board")).toBe(false);
  });

  it("fails when the code fails to load, as lazy() expects", async () => {
    vi.resetModules();
    const { withText } = await import("./prefetch");
    await expect(withText([], () => Promise.reject(new Error("chunk")))()).rejects.toThrow("chunk");
  });
});
