// Which way the window reads (mu-149), in one place: the language shown decides it, as it decides
// `<html dir>` (i18n/runtime.ts, applyToDocument). The splitters' drag, the arrow keys that mean
// next and previous and the Feature grid ask here, never a direction of their own. Tested in
// rtl.test.ts.
import { dirOf, getLocale } from "./i18n";

/** True while the window reads right to left (فارسی). */
export const isRtl = (): boolean => dirOf(getLocale()) === "rtl";

const SWAPPED: Record<string, string> = { ArrowLeft: "ArrowRight", ArrowRight: "ArrowLeft" };

/** An arrow key that means next or previous, as it would be pressed left to right: right to left
 *  ← and → swap, since they follow the reading direction there (← is next). Any other key is
 *  itself. Not for the splitters, whose arrows stay physical. */
export const readingArrow = (key: string, rtl: boolean): string => (rtl ? (SWAPPED[key] ?? key) : key);
