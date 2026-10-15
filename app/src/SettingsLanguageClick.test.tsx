// Settings › Language's chips when clicked (mu-146, review R7). The window only shows a language
// once its files are in, so a quick فارسی → English change of mind arrives while English is
// still the one shown: that click has to reach the setting too, or Persian wins. `t` is stubbed
// so the view can be walked as plain elements (no DOM here).
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./i18n/react", async (real) => ({ ...(await real<typeof import("./i18n/react")>()), useT: () => (k: string) => k }));

const { LanguageView, NumberingView } = await import("./Settings");

type Props = { children?: ReactNode; onClick?: () => void; lang?: string };

function elements(node: ReactNode): ReactElement<Props>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Props>(node)) return [];
  if (typeof node.type === "function") return elements((node.type as (p: Props) => ReactNode)(node.props));
  return [node, ...elements(node.props.children)];
}

const chip = (tree: ReactElement<Props>[], lang: string) => tree.find((e) => e.type === "button" && e.props.lang === lang);

describe("the Language chips, clicked", () => {
  it("pass a change of mind back to the shown language while the other one loads", () => {
    const onPick = vi.fn();
    // English shown and saved; فارسی clicked, its files not in yet, so English is still shown.
    const tree = elements(LanguageView({ shown: "en", following: false, onPick }));
    chip(tree, "fa")?.props.onClick?.();
    chip(tree, "en")?.props.onClick?.();
    expect(onPick.mock.calls).toEqual([["fa"], ["en"]]);
  });

  it("Digits and Calendar (mu-151) pass each click on, the one on too", () => {
    const onDigits = vi.fn();
    const onCalendar = vi.fn();
    const tree = elements(NumberingView({ digits: "persian", calendar: "jalali", onDigits, onCalendar }));
    tree.filter((e) => e.type === "button").forEach((b) => b.props.onClick?.());
    expect(onDigits.mock.calls).toEqual([["persian"], ["latin"]]);
    expect(onCalendar.mock.calls).toEqual([["jalali"], ["gregorian"]]);
  });

  it("pin the shown language when it's macOS's", () => {
    const onPick = vi.fn();
    const tree = elements(LanguageView({ shown: "fa", following: true, onPick }));
    chip(tree, "fa")?.props.onClick?.();
    expect(onPick).toHaveBeenCalledWith("fa");
  });
});
