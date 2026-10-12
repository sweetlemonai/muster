import { describe, expect, it } from "vitest";
import { ACCENTS, contrast, DEFAULT_CHOICE, mix, resolve, THEMES, variables } from "./themes";

describe("every theme and accent pair is readable (WCAG AA)", () => {
  for (const t of THEMES) {
    it(`${t.name}: text and secondary text`, () => {
      for (const surface of [t.bg, t.surface, t.popover]) {
        expect(contrast(t.text, surface)).toBeGreaterThanOrEqual(7);
        expect(contrast(t.muted, surface)).toBeGreaterThanOrEqual(4.5);
      }
    });
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name}: accent and text on it`, () => {
        const v = variables(t, a);
        // The accent is for fills, borders and selection: 3:1 against what's around it.
        expect(contrast(v["--accent"], t.bg)).toBeGreaterThanOrEqual(3);
        expect(contrast(v["--accent"], t.surface)).toBeGreaterThanOrEqual(3);
        // Accent-coloured words use --accent-text: 4.5:1 on every surface.
        for (const surface of [t.bg, t.surface, t.popover]) expect(contrast(v["--accent-text"], surface)).toBeGreaterThanOrEqual(4.5);
        // Button text on the accent.
        expect(contrast(v["--on-accent"], v["--accent"])).toBeGreaterThanOrEqual(4.5);
        // Text on the soft accent (selected chips).
        expect(contrast(t.text, v["--accent-soft"])).toBeGreaterThanOrEqual(4.5);
      });
    }
    it(`${t.name}: coloured status words`, () => {
      const v = variables(t, ACCENTS[0]);
      for (const k of ["--err", "--ok", "--gold-text"]) expect(contrast(v[k], t.surface)).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe("choices", () => {
  it("defaults to Muster's violet, following the Mac", () => {
    expect(resolve(DEFAULT_CHOICE, true).theme.id).toBe("violet-night");
    expect(resolve(DEFAULT_CHOICE, false).theme.id).toBe("violet-day");
    expect(resolve(DEFAULT_CHOICE, true).accent.id).toBe("violet");
  });
  it("a fixed theme ignores the Mac", () => {
    expect(resolve({ ...DEFAULT_CHOICE, matchSystem: false, theme: "paper" }, true).theme.id).toBe("paper");
  });
  it("unknown ids fall back", () => {
    const r = resolve({ ...DEFAULT_CHOICE, matchSystem: false, theme: "nope", accent: "nope" }, true);
    expect([r.theme.id, r.accent.id]).toEqual(["violet-night", "violet"]);
  });
  it("mix", () => {
    expect(mix("#FFFFFF", "#000000", 0.5)).toBe("#808080");
  });
});
