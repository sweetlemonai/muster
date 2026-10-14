# 0001 — The Muster app: Drover's stack, in `app/`, reading every `~/playground/*/tasks`

Date: 2026-10-12 · Status: accepted (owner: "for now let's do a different app", Muster to become
part of Drover later)

## Decision

- **Stack:** Tauri 2.12 + React 19 + Vite 8 + TypeScript, pnpm. That's Drover's stack, so screens
  and Rust code can move into Drover later without a rewrite.
- **Dependencies:** only ones Drover already uses (`tauri`, `tauri-plugin-dialog` for the folder
  picker, called from Rust only, so the window gets no dialog permission; `serde`, `serde_json`, `tempfile` for
  tests; `react`, `react-dom`, `@tauri-apps/api`, `markdown-it`). **Amended 2026-10-13 (task
  0052):** plus `tauri-plugin-notification` 2.6.0 (Drover's version; the owner said yes), called
  from Rust only, so the window gets no notification permission either. The Rust lockfile was seeded
  from Drover's so the versions match. No libc: file locking uses std's `File::lock` (Rust
  1.89+), and the answer's time stamp comes from the window's local clock, validated in Rust.
- **Layout:** the app lives in `app/` inside the Muster repo (`app/src`, `app/src-tauri`),
  beside the Python tools in `bin/` that agents use. One repo, two front doors.
- **Queues:** found by looking for `~/playground/*/tasks` with a `README.md` and a `todo/` folder
  (or a `tasks.json`). The window names a queue by project, never by path. Both the v2 layout
  (status folders) and the flat layout (status only in the file) are read, so the planned move
  to files that never move needs no app change.
- **Live:** the window polls every 2 s while visible and stops when hidden. Parsed files are
  cached by size and mtime, and git logs by the mtime of `.git/logs/HEAD`. Measured on the real
  queues (21 tasks): 19 ms cold, 0.6 ms warm. No file-watcher dependency.
  **Amended 2026-10-13 (task 0006):** the window still stops polling while hidden, but the core
  now reads the queues every 15 s while it is, for the Dock badge alone (about 7 ms warm on 662
  tasks), plus at most one `claude agents` run per 15 s (0048) while someone holds a task in
  Building or Testing with a session stamp. The same read, and each poll, also decide the Mac
  notifications (0052): no extra reads.
- **Writes:** only the owner's answers, through file contract v1 (`docs/PROTOCOL.md`), proven
  against `bin/task` by `tests/interop.rs`. The app never runs a script from a queue.
- **Safety:** task text is rendered with markdown-it `html: false`, and links don't navigate.
  Images come only from `attachments/<id>/` (plain names, regular files, under 15 MB, png, jpg,
  gif or webp). The capability is `core:default` only. CSP as Drover's.
- **"Stale":** a claimed task whose file hasn't changed in 12 h, from the file's mtime, not
  the `heartbeat:` text, because agents' typed times aren't trusted (`DOGFOOD.md` 37).
  **Amended 2026-10-12 (task 0002):** `heartbeat:` is now written only by `muster` (claim,
  `heartbeat`, and the holder's notes, passes and questions; and an unblock), so it's trusted. Stale
  means quiet past the 12 h lease by that heartbeat, falling back to mtime when it's empty. Reclaim,
  `check`, the card and Release all use the same rule.

## Options considered

- **A local web page served by a Python script** (no build step): quick, but it needs a port and a
  browser tab, and nothing of it moves into Drover.
- **A SwiftUI app:** native, but a third stack, and none of it moves into Drover.
- **Inside Drover right away:** the end state, but it would wait on Drover's Settings work (0008)
  and its review cadence. The owner wanted to watch Muster now.

## Revisit when

The screens move into Drover (Drover task 0005/0015), or polling shows up in profiles with many
queues (then: FSEvents through a crate Drover adopts).
