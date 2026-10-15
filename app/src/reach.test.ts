import { describe, expect, it } from "vitest";
import pingRs from "../src-tauri/src/ping.rs?raw";
import { NO_SESSION, NO_SESSION_HELD, noSession, unreachable } from "./reach";

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
});
