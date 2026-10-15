import { afterEach, describe, expect, it } from "vitest";
import { setLocale } from "./i18n/runtime";
import { isRtl, readingArrow } from "./rtl";

// The one place the window asks which way it reads (mu-149): the splitters, the arrow keys and the
// Feature grid ask it rather than each reading a direction of their own.

describe("which way the window reads", () => {
  afterEach(async () => {
    await setLocale("en");
  });

  it("is left to right in English and right to left in فارسی, following the language as it switches", async () => {
    expect(isRtl()).toBe(false);
    await setLocale("fa");
    expect(isRtl()).toBe(true);
    await setLocale("en");
    expect(isRtl()).toBe(false);
  });
});

describe("the window's direction, read in one place", () => {
  // Every source file of the window, as text, but the tests and this helper's own.
  const sources = Object.entries(import.meta.glob<string>(["./**/*.{ts,tsx}", "!./**/*.test.{ts,tsx}", "!./rtl.ts"], { query: "?raw", import: "default", eager: true }));
  it("is never read by a component of its own: no computed style's direction, no document's dir (only the language layer sets it)", () => {
    const reads = sources.filter(([, text]) => /\.direction\b|\bdocument\.dir\b|documentElement\.dir\b(?!\s*=[^=])/.test(text)).map(([path]) => path);
    expect(reads).toEqual([]);
  });
  it("is asked of isRtl by the splitter, the sidebar's headings, the Feature grid and the board's cue", () => {
    const ask = Object.fromEntries(sources.filter(([path]) => ["./ui/Splitter.tsx", "./SidebarProjects.tsx", "./FeatureGrid.tsx", "./ColumnRow.tsx"].includes(path)).map(([path, text]) => [path, text.includes("isRtl()")]));
    expect(ask).toEqual({ "./ui/Splitter.tsx": true, "./SidebarProjects.tsx": true, "./FeatureGrid.tsx": true, "./ColumnRow.tsx": true });
  });
});

describe("an arrow key that means next or previous", () => {
  it("is itself left to right, and ← and → swap right to left; nothing else changes", () => {
    expect(["ArrowLeft", "ArrowRight", "ArrowUp", "Enter"].map((k) => readingArrow(k, false))).toEqual(["ArrowLeft", "ArrowRight", "ArrowUp", "Enter"]);
    expect(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "a"].map((k) => readingArrow(k, true))).toEqual(["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown", "Home", "a"]);
  });
});
