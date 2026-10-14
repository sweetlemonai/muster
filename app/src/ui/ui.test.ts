import { createElement, createRef, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Badge, type BadgeVariant, Button, Pill, type PillStatus, Segment, Segmented, StatusPill } from ".";
import { STAGES, type StageId } from "../lib";
import css from "../styles.css?raw";

const html = (node: ReactNode) => renderToStaticMarkup(node);

describe("Button", () => {
  it("has its root class and variant, as Drover's", () => {
    expect(html(createElement(Button, null, "Go"))).toBe('<button class="ui-button">Go</button>');
    expect(html(createElement(Button, { variant: "primary" }, "Go"))).toBe('<button class="ui-button ui-button--primary">Go</button>');
    expect(html(createElement(Button, { variant: "primary", className: "new" }, "+ New task"))).toBe('<button class="ui-button ui-button--primary new">+ New task</button>');
  });

  it("passes native props through, and forces no type", () => {
    const out = html(createElement(Button, { disabled: true, title: "t", "aria-pressed": true, type: "submit" }, "Save"));
    expect(out).toBe('<button disabled="" title="t" aria-pressed="true" type="submit" class="ui-button">Save</button>');
    // Without one, a button in a form still submits, as before.
    expect(html(createElement(Button, null, "Send"))).not.toContain("type=");
  });

  it("hands its ref to the <button> (a prop in React 19; static markup never attaches one)", () => {
    const ref = createRef<HTMLButtonElement>();
    const el = Button({ ref, children: "Save" });
    expect(el.type).toBe("button");
    expect(el.props.ref).toBe(ref);
  });
});

describe("Pill", () => {
  it("is a rounded chip, `on` when it's the chosen one", () => {
    expect(html(createElement(Pill, null, "All"))).toBe('<button class="ui-pill">All</button>');
    expect(html(createElement(Pill, { on: true, "aria-pressed": true }, "All"))).toBe('<button aria-pressed="true" class="ui-pill ui-pill--on">All</button>');
    expect(html(createElement(Pill, { on: false, className: "add" }, "+ Save view"))).toBe('<button class="ui-pill add">+ Save view</button>');
  });
});

describe("StatusPill", () => {
  it("says a stage in its colour, or blocked, or paused", () => {
    expect(html(createElement(StatusPill, { status: "building" }, "Building"))).toBe('<span class="ui-status-pill ui-status-pill--building">Building</span>');
    expect(html(createElement(StatusPill, { status: "your-check" }, "Your check"))).toBe('<span class="ui-status-pill ui-status-pill--your-check">Your check</span>');
    expect(html(createElement(StatusPill, { status: "blocked", className: "block-why" }, "blocked"))).toBe('<span class="ui-status-pill ui-status-pill--blocked block-why">blocked</span>');
    expect(html(createElement(StatusPill, { status: "paused", title: "why" }, "Paused"))).toBe('<span title="why" class="ui-status-pill ui-status-pill--paused">Paused</span>');
  });
});

describe("Badge", () => {
  it("is a small label, with a variant for what it says", () => {
    expect(html(createElement(Badge, null, "2 open Q"))).toBe('<span class="ui-badge">2 open Q</span>');
    expect(html(createElement(Badge, { variant: "warn" }, "needs you"))).toBe('<span class="ui-badge ui-badge--warn">needs you</span>');
    expect(html(createElement(Badge, { variant: "est", title: "Estimated" }, "1h"))).toBe('<span title="Estimated" class="ui-badge ui-badge--est">1h</span>');
    expect(html(createElement(Badge, { variant: "blocked", className: "block-why" }, "x"))).toBe('<span class="ui-badge ui-badge--blocked block-why">x</span>');
  });
});

describe("Segmented and Segment", () => {
  it("is a group of joined buttons, the active one pressed", () => {
    const out = html(
      createElement(
        Segmented,
        { "aria-label": "Priority" },
        createElement(Segment, { active: true }, "high"),
        createElement(Segment, null, "low"),
      ),
    );
    expect(out).toBe(
      '<div role="group" aria-label="Priority" class="ui-segmented">' +
        '<button aria-pressed="true" class="ui-segment ui-segment--active">high</button>' +
        '<button aria-pressed="false" class="ui-segment">low</button>' +
        "</div>",
    );
  });

  it("takes its own element and attributes", () => {
    expect(html(createElement(Segmented, { className: "x" }))).toBe('<div role="group" class="ui-segmented x"></div>');
    // A caller's pressed state wins over the default.
    expect(html(createElement(Segment, { "aria-pressed": undefined }, "a"))).toBe('<button class="ui-segment">a</button>');
  });
});

describe("styles.css", () => {
  // Every class an atom can put on the page, from its types: a missing key is a type error.
  const flags: Record<Exclude<PillStatus, StageId>, true> = { blocked: true, paused: true };
  const variants: Record<BadgeVariant, true> = { warn: true, blocked: true, paused: true, note: true, est: true, level: true };
  const selectors = css.replace(/\/\*[\s\S]*?\*\//g, "").match(/[^{}]+(?=\{)/g)?.join(",") ?? "";
  const styled = (cls: string) => new RegExp(`\\.${cls}(?![\\w-])`).test(selectors);

  it("has a rule for every status a StatusPill can say", () => {
    for (const s of [...STAGES.map((st) => st.id), ...Object.keys(flags)]) expect(styled(`ui-status-pill--${s}`), s).toBe(true);
  });
  it("has a rule for every Badge variant", () => {
    for (const v of Object.keys(variants)) expect(styled(`ui-badge--${v}`), v).toBe(true);
  });
  it("and the check sees a missing one", () => {
    expect(styled("ui-badge--nope")).toBe(false);
    expect(styled("ui-status-pill--your")).toBe(false);
  });
});
