# Stream fixtures

Real `claude -p --output-format stream-json --verbose` runs from Claude Code 2.1.296, recorded by
the mu-111 spike on 2026-10-14 and trimmed: hook lines dropped, `init` cut to a few keys, ids and
paths replaced (`session-1`, `uuid-N`, `msg_redacted`, `~/scratch`). The findings are in mu-9's
comments.

- `claude-rate-limit-allowed.jsonl`: a one-turn run on a signed-in account. `rate_limit_event`
  comes after the assistant message, with status `allowed`.
- `claude-not-logged-in.jsonl`: the same run on an account that isn't logged in. Exit code 1, a
  synthetic assistant message with `error: "authentication_failed"`, and a `result` whose
  `subtype` is still `success` but with `is_error: true` and `terminal_reason: "api_error"`.

A rejected run (`status: "rejected"`) wasn't seen live; its shape from the 2.1.296 binary is in
the mu-9 comment.
