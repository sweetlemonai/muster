import { describe, expect, it } from "vitest";
import { commentDone } from "./Comments";

describe("what a sent comment says it did (mu-101)", () => {
  it("a plain comment is added; one that acted says what it did", () => {
    expect(commentDone({ path: "doing/0082-t.md", did: "", unsaved: "" })).toBe("Comment added.");
    expect(commentDone({ path: "doing/0082-t.md", did: "sent back to Building (owner finding R1)", unsaved: "" })).toBe("Comment added: sent back to Building (owner finding R1).");
    expect(commentDone({ path: "done/0082-t.md", did: "accepted (Done)", unsaved: "" })).toBe("Comment added: accepted (Done).");
    expect(commentDone({ path: "doing/0082-t.md", did: "blocked on mu-12, mu-14", unsaved: "" })).toBe("Comment added: blocked on mu-12, mu-14.");
  });
  it("an action whose comment couldn't be saved says it ran, and not to send it again (R13)", () => {
    expect(commentDone({ path: "doing/0082-t.md", did: "blocked (outside)", unsaved: "task file is over 1 MB" })).toBe(
      "Done: blocked (outside). The comment couldn't be saved (task file is over 1 MB), so it isn't in the thread; don't send it again.",
    );
  });
});
