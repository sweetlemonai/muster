import { describe, expect, it } from "vitest";
import { commentDone } from "./Comments";

describe("what a sent comment says it did (mu-101)", () => {
  it("a plain comment is added; one that acted says what it did", () => {
    expect(commentDone({ path: "doing/0082-t.md", did: "" })).toBe("Comment added.");
    expect(commentDone({ path: "doing/0082-t.md", did: "sent back to Building (owner finding R1)" })).toBe("Comment added: sent back to Building (owner finding R1).");
    expect(commentDone({ path: "done/0082-t.md", did: "accepted (Done)" })).toBe("Comment added: accepted (Done).");
    expect(commentDone({ path: "doing/0082-t.md", did: "blocked on mu-12, mu-14" })).toBe("Comment added: blocked on mu-12, mu-14.");
  });
});
