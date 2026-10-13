import { describe, expect, it } from "vitest";
import { ACCENTS, contrast, DEFAULT_CHOICE, luminance, mix, resolve, STAGE_COLOURS, THEMES, variables } from "./themes";

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
    it(`${t.name}: words on stage colours (pills, badges)`, () => {
      const v = variables(t, ACCENTS[0]);
      for (const s of Object.keys(STAGE_COLOURS)) expect(contrast(v["--on-status"], v[`--st-${s}`]), s).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe("stage colours", () => {
  const hsl = (hex: string) => {
    const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex.slice(1 + i, 3 + i), 16) / 255);
    const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min, l = (max + min) / 2;
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
    const h = d === 0 ? 0 : max === r ? 60 * (((g - b) / d + 6) % 6) : max === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4);
    return { h, s };
  };
  it("cover every stage and blocked, each its own colour", () => {
    expect(Object.keys(STAGE_COLOURS)).toEqual(["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "blocked"]);
    expect(new Set(Object.values(STAGE_COLOURS)).size).toBe(8);
  });
  it("are never orange or blue (those belong to other apps)", () => {
    for (const [stage, c] of Object.entries(STAGE_COLOURS)) {
      const { h, s } = hsl(c);
      if (s < 0.2) continue; // grey
      expect(h >= 15 && h <= 38, `${stage} ${c} is orange`).toBe(false);
      expect(h >= 195 && h <= 245, `${stage} ${c} is blue`).toBe(false);
    }
  });
  it("Shaping and Ready are violet, Ready the stronger; Building and Testing teal, Testing the lighter", () => {
    for (const s of ["shaping", "ready"]) expect(hsl(STAGE_COLOURS[s]).h).toBeGreaterThan(245);
    for (const s of ["building", "testing"]) expect(Math.abs(hsl(STAGE_COLOURS[s]).h - 174)).toBeLessThan(10);
    expect(luminance(STAGE_COLOURS.ready)).toBeLessThan(luminance(STAGE_COLOURS.shaping));
    expect(luminance(STAGE_COLOURS.testing)).toBeGreaterThan(luminance(STAGE_COLOURS.building));
  });
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
