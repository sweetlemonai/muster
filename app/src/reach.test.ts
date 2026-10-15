import { afterEach, describe, expect, it } from "vitest";
import pingRs from "../src-tauri/src/ping.rs?raw";
import { unmarked } from "./i18n";
import common from "./i18n/locales/en/common.json";
import { setLocale } from "./i18n/runtime";
import { askedText, cantAsk, NO_SESSION, NO_SESSION_HELD, noSession, unreachable } from "./reach";
import { answerFirst } from "./sessionState";
import type { Task } from "./types";

// The window says what the core says (mu-184): a disabled Ping's reason is ping::NO_SESSION.

describe("NO_SESSION, kept in step with ping.rs", () => {
  it("is ping::NO_SESSION word for word, {label} and all (a source check)", () => {
    const rust = /pub const NO_SESSION: &str =\s*"([^"]*)";/.exec(pingRs)?.[1];
    expect(rust).toBe(NO_SESSION);
    const held = /pub const NO_SESSION_HELD: &str =\s*"([^"]*)";/.exec(pingRs)?.[1];
    expect(held).toBe(NO_SESSION_HELD);
  });
  it("offers a takeover only where one can happen: Building, not a hold in Testing", () => {
    expect(unreachable({ stage: "building", claimedBy: "Asghar_7", heldBy: "" })).toBe(noSession("Asghar_7"));
    expect(unreachable({ stage: "testing", claimedBy: "", heldBy: "Asghar_7" })).toBe("no session to reach: Release it");
  });
  it("says what the owner can do, with the holder's label", () => {
    expect(noSession("Asghar_7")).toBe("no session to reach: Release it, or let a new session under Asghar_7 take it over");
    // A label with $ patterns is put in as is.
    expect(noSession("a$&b")).toBe("no session to reach: Release it, or let a new session under a$&b take it over");
  });
  it("is the window's English word for word, so the core and the window say the same (mu-147)", () => {
    expect(common.reach.noSession).toBe(NO_SESSION.replace("{label}", "{{label}}"));
    expect(common.reach.noSessionHeld).toBe(NO_SESSION_HELD);
  });
});

describe("why a session can't be reached, in Persian (mu-147)", () => {
  afterEach(async () => {
    await setLocale("en");
  });
  const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";
  const t = (over: Partial<Pick<Task, "stage" | "claimedBy" | "heldBy" | "sessionState">>) => ({ stage: "building", claimedBy: "", heldBy: "", ...over });
  it("says each reason in Persian, the holder's label as written", async () => {
    await setLocale("fa");
    expect(unreachable(t({ claimedBy: A }))).toBe("");
    expect(unmarked(noSession("Asghar_7"))).toBe("نشستی برای دسترسی نیست: رهایش کنید، یا بگذارید نشستی تازه با نام Asghar_7 آن را به دست بگیرد");
    expect(unreachable(t({ claimedBy: "Asghar_7" }))).toBe(noSession("Asghar_7"));
    expect(unreachable(t({ stage: "testing", heldBy: "Asghar_7" }))).toBe("نشستی برای دسترسی نیست: رهایش کنید");
    expect(unreachable(t({}))).toBe("کسی آن را در دست ندارد");
    expect(unreachable(t({ claimedBy: "owner (session 7b5e8856)" }))).toBe("خودتان آن را در دست دارید");
    expect(unmarked(unreachable(t({ stage: "ready", claimedBy: A })))).toBe("در «آماده» نشستی روی آن کار نمی‌کند");
    expect(cantAsk(t({ stage: "testing", heldBy: A }))).toBe("گزارش وضعیت مخصوص «در حال ساخت» است؛ به‌جایش پیام بفرستید");
    expect(cantAsk(t({ claimedBy: A, sessionState: "needs-input" }))).toBe(answerFirst());
    expect(answerFirst()).toBe("در انتظار ورودی در Claude Code: اول همان‌جا پاسخ دهید");
  });
  it("says when a check-in was asked for in Persian", async () => {
    const at = (s: string) => {
      const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
      return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
    };
    const asked = { stage: "building", stageSince: "2026-10-13 09:00", checkins: [], checkinAsked: "2026-10-13 10:38" };
    expect(askedText(asked, at("2026-10-13 10:40"))).toBe("asked 2 min ago");
    await setLocale("fa");
    expect(unmarked(askedText(asked, at("2026-10-13 10:40")))).toBe("۲ دقیقه پیش درخواست شد");
    expect(unmarked(askedText(asked, at("2026-10-13 10:38")))).toBe("لحظاتی پیش درخواست شد");
  });
});
