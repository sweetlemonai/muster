// The layer's React side. Muster's tests run without a DOM (vitest's node
// environment, no jsdom), so components render to markup with react-dom's
// server renderer; Drover's tests mount them in jsdom instead.
import { memo } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PSEUDO } from "./locales";
import { renderTags, Trans, useLocale, useT } from "./react";
import bidiCases from "./fixtures/bidi-controls.json";
import { setLocale } from "./runtime";

afterEach(async () => {
  await setLocale("en");
});

const html = (el: React.ReactElement) => renderToStaticMarkup(el);
const text = (el: React.ReactElement) => html(el).replace(/<[^>]+>/g, "").replace(/&#x27;/g, "'");

function Name() {
  const t = useT("shell");
  return <span>{t("language.label")}</span>;
}

// Memoized with no props: only its own subscription can re-render it.
const Memo = memo(function Memo() {
  const t = useT("native");
  return <b>{t("tray.waiting", { count: 2 })}</b>;
});

function Shown() {
  return <i>{useLocale()}</i>;
}

describe("useT", () => {
  it("renders the text in the language shown, memoized parts too", async () => {
    const page = (
      <p>
        <Name />
        <Memo />
        <Shown />
      </p>
    );
    expect(text(page)).toBe("Language2 things wait for youen");
    await setLocale(PSEUDO);
    expect(text(page)).toBe("[Ļáñĝûáĝé ~~~][2 ţĥîñĝš ŵáîţ ƒöŕ ýöû ~~~~~~~]pseudo");
    await setLocale("fa");
    expect(text(page)).toBe("زبان۲ مورد منتظر شماستfa");
  });

  it("loads a namespace that isn't in yet, shows nothing for that moment, then its text", async () => {
    // A fresh copy of the layer with nothing loaded, as a part outside its
    // screen's own text might find it.
    vi.resetModules();
    const fresh = await import("./react");
    const runtime = await import("./runtime");
    runtime.setReader((await import("./testReader")).readFromDisk);
    function Fresh() {
      const t = fresh.useT("shell");
      return <span>{t("language.label")}</span>;
    }
    expect(runtime.isLoaded("en", "shell")).toBe(false);
    // No key on screen while the file loads, and no Suspense needed.
    expect(html(<Fresh />)).toBe("<span></span>");
    await vi.waitFor(() => expect(runtime.isLoaded("en", "shell")).toBe(true));
    expect(html(<Fresh />)).toBe("<span>Language</span>");
  });

  it("checks keys at compile time", () => {
    function Typo() {
      const t = useT("shell");
      // @ts-expect-error: not a key in shell.json
      return <span>{t("language.lable")}</span>;
    }
    expect(Typo).toBeTypeOf("function");
  });
});

describe("Trans", () => {
  it("puts elements where the tags are, and keeps a value's text as text", () => {
    const out = html(
      <p>
        {renderTags("Run <code>{{cmd}}</code> then <b>stop</b><br/>.", { code: <code className="x" />, b: <strong />, br: <br /> }, (s) => s.replace("{{cmd}}", "<b>pnpm</b>"))}
      </p>,
    );
    expect(out).toBe('<p>Run <code class="x">&lt;b&gt;pnpm&lt;/b&gt;</code> then <strong>stop</strong><br/>.</p>');
  });

  it("drops a tag nobody supplied, keeping its text", () => {
    expect(html(<p>{renderTags("a <x>b</x> c", {})}</p>)).toBe("<p>a b c</p>");
  });

  it("translates through a namespace", () => {
    expect(html(<Trans ns="shell" k="language.failed" params={{ error: "<i>disk</i>" }} components={{}} />)).toBe(
      "Couldn&#x27;t change the language: &lt;i&gt;disk&lt;/i&gt;",
    );
  });

  // Right to left (Drover's task 0020): a value is isolated by an element,
  // never by characters, so copying a path copies exactly that.
  it("isolates values with <bdi> in Persian", async () => {
    await setLocale("fa");
    const out = html(<Trans ns="shell" k="language.failed" params={{ error: "~/w/settings.json" }} components={{}} />);
    expect(out).toBe("تغییر زبان ممکن نشد: <bdi>~/w/settings.json</bdi>");
    expect(out).not.toMatch(/[\u2066-\u2069]/);
  });

  it("adds no <bdi> left to right", () => {
    const out = html(<Trans ns="shell" k="language.failed" params={{ error: "disk" }} components={{}} />);
    expect(out).toBe("Couldn&#x27;t change the language: disk");
  });

  // mu-152 AC9: a value in its <bdi> loses its bidi controls as a filled-in
  // placeholder does (runtime.cleanValue), from the cases i18n.rs reads too.
  it.each(bidiCases.cases)("cleans a value in its <bdi>, from $about", async ({ value, window }) => {
    const line = () => html(<Trans ns="native" k="notify.line.own" params={{ id: "mu-1", title: value }} components={{}} />);
    expect(line()).toBe(`mu-1 ${window}: your task`);
    await setLocale("fa");
    expect(line()).toBe(`<bdi>mu-1</bdi> ${window ? `<bdi>${window}</bdi>` : ""}: تسک خودتان`);
  });

  it("cleans each bidi control from a value in its <bdi>", async () => {
    await setLocale("fa");
    for (const c of bidiCases.controls.map((x) => String.fromCodePoint(Number.parseInt(x.slice(2), 16)))) {
      const out = html(<Trans ns="native" k="notify.line.own" params={{ id: `mu${c}-1`, title: c }} components={{}} />);
      expect(out, x(c)).toBe("<bdi>mu-1</bdi> : تسک خودتان");
    }
  });
});

const x = (c: string) => `U+${c.codePointAt(0)?.toString(16).toUpperCase()}`;
