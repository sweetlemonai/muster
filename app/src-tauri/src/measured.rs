//! Estimates against what finished tasks took (mu-168), for the shaping brief: actual ÷ `est-time`
//! at p50 and p80 and the median hours, overall, by type and by size of the estimate, from each
//! task's stage history. Pure, no I/O: `backlog::brief` hands it the tasks it already read.
//!
//! Interim: mu-115's calibration (on mu-114's ledger) replaces it, and this module goes then. Named
//! `measured`, not `actuals`, so it isn't taken for 0017's `transcripts::actuals`.

use crate::queue::{StageStep, Task};
use serde::Serialize;

/// A cell with fewer finished tasks than this says "too few (n)" instead of a number.
pub const MIN_CELL: usize = 5;

/// Sizes of the estimate, each up to its minutes (1d = 480 min, as `ops::time_minutes` reads it).
pub const SIZE_S: u64 = 60;
pub const SIZE_M: u64 = 180;
pub const SIZE_L: u64 = 480;
/// The sizes in order, with their names in the brief.
pub const SIZES: [(&str, &str); 4] = [
    ("S", "S (up to 1h)"),
    ("M", "M (up to 3h)"),
    ("L", "L (up to 8h)"),
    ("XL", "XL (over 8h)"),
];

/// Which estimate is which, said under the numbers.
pub const RULE: &str = "--time is agent time: the first claim to leaving Testing, review rounds and QA included, time blocked or paused left out; --test-time is the owner's check only (QA is in --time); the owner's wait in Your check is neither";

/// What the brief says when nothing has finished yet.
pub const NONE_YET: &str = "no finished tasks with an estimate yet: estimate by scope";

/// One group's numbers. `p50`, `p80` and `median_hours` are None when `n` is under `MIN_CELL`
/// ("too few (n)"). Ratios are rounded to 2 places and hours to 1, so `--json` and the text say
/// the same numbers.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    /// "all", the type ("code") or the size ("S").
    pub name: String,
    pub n: usize,
    pub p50: Option<f64>,
    pub p80: Option<f64>,
    pub median_hours: Option<f64>,
}

/// The owner's wait in Your check: entering it → done, kept apart from agent time.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wait {
    pub n: usize,
    /// None under `MIN_CELL` tasks.
    pub median_hours: Option<f64>,
}

/// The queue's finished tasks measured against their estimates.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    pub overall: Cell,
    /// Each type with a finished task, most tasks first (then by name).
    pub by_type: Vec<Cell>,
    /// S, M, L, XL, always all four.
    pub by_size: Vec<Cell>,
    pub owner_wait: Wait,
}

/// A stamp's minutes, or None for one that isn't a tool stamp. Naive local time, no zone, so any
/// TZ reads the same spans.
fn at(s: &StageStep) -> Option<i64> {
    crate::ops::stamp_minutes(&s.at)
}

/// Minutes of `start..end` a task was blocked or paused (`block_history`, `pause_history`), each
/// minute once where the two overlap; a stretch still open runs to `end`.
fn stopped_within(t: &Task, start: i64, end: i64) -> i64 {
    let blocks = t.block_history.iter().map(|b| (&b.from, &b.to));
    let pauses = t.pause_history.iter().map(|p| (&p.from, &p.to));
    let mut spans: Vec<(i64, i64)> = blocks
        .chain(pauses)
        .filter_map(|(from, to)| {
            let a = crate::ops::stamp_minutes(from)?;
            let z = if to.is_empty() {
                end
            } else {
                crate::ops::stamp_minutes(to)?
            };
            let (a, z) = (a.max(start), z.min(end));
            (z > a).then_some((a, z))
        })
        .collect();
    spans.sort_unstable();
    let mut total = 0;
    let mut upto = start;
    for (a, z) in spans {
        let a = a.max(upto);
        if z > a {
            total += z - a;
            upto = z;
        }
    }
    total
}

/// A task's agent time in minutes: its first move into Building → its last move out of Testing to
/// Your check or Done after it, send-backs, review rounds and QA included, less the time it was
/// blocked or paused (nobody works then). None when it never got that far, when it went back
/// after that (sent back from Your check or reopened: not finished until it leaves Testing
/// again), or the span is zero or backwards (bad stamps, or blocked throughout).
pub fn agent_minutes(t: &Task) -> Option<i64> {
    let h = &t.stage_history;
    let first = h.iter().position(|s| s.to == "building")?;
    let left = first
        + 1
        + h[first + 1..]
            .iter()
            .rposition(|s| s.from == "testing" && (s.to == "your-check" || s.to == "done"))?;
    if !h[left + 1..]
        .iter()
        .all(|s| s.to == "your-check" || s.to == "done")
    {
        return None;
    }
    let (start, end) = (at(&h[first])?, at(&h[left])?);
    let worked = end - start - stopped_within(t, start, end);
    (end > start && worked > 0).then_some(worked)
}

/// A task's wait for the owner in minutes: its last move to Done, when that came from Your check,
/// from its last move into Your check before it. None when it never waited there (Done from
/// another stage: an override or a hand move), or the span is zero or backwards.
pub fn owner_minutes(t: &Task) -> Option<i64> {
    let h = &t.stage_history;
    let done = h.iter().rposition(|s| s.to == "done")?;
    if h[done].from != "your-check" {
        return None;
    }
    let entered = h[..done].iter().rev().find(|s| s.to == "your-check")?;
    let (start, end) = (at(entered)?, at(&h[done])?);
    (end > start).then_some(end - start)
}

/// The size an estimate of `minutes` falls in.
pub fn size(minutes: u64) -> &'static str {
    match minutes {
        m if m <= SIZE_S => "S",
        m if m <= SIZE_M => "M",
        m if m <= SIZE_L => "L",
        _ => "XL",
    }
}

/// The value at quantile `q` of sorted `v`, by nearest rank (the ⌈q·n⌉-th, at least the first).
fn nearest_rank(v: &[f64], q: f64) -> f64 {
    let rank = (q * v.len() as f64).ceil() as usize;
    v[rank.clamp(1, v.len()) - 1]
}

fn round(x: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (x * f).round() / f
}

/// One finished task: actual ÷ estimate, actual hours, type, estimate in minutes.
struct Done {
    ratio: f64,
    hours: f64,
    kind: String,
    est: u64,
}

fn cell(name: &str, rows: &[&Done]) -> Cell {
    let n = rows.len();
    if n < MIN_CELL {
        return Cell {
            name: name.to_string(),
            n,
            p50: None,
            p80: None,
            median_hours: None,
        };
    }
    let mut ratios: Vec<f64> = rows.iter().map(|d| d.ratio).collect();
    let mut hours: Vec<f64> = rows.iter().map(|d| d.hours).collect();
    ratios.sort_by(f64::total_cmp);
    hours.sort_by(f64::total_cmp);
    Cell {
        name: name.to_string(),
        n,
        p50: Some(round(nearest_rank(&ratios, 0.5), 2)),
        p80: Some(round(nearest_rank(&ratios, 0.8), 2)),
        median_hours: Some(round(nearest_rank(&hours, 0.5), 1)),
    }
}

/// The queue's `tasks` measured: each with an `est-time` and a finished agent span counts (see
/// `agent_minutes`); every task that waited in Your check counts for `owner_wait`.
pub fn measure(tasks: &[Task]) -> Measured {
    let done: Vec<Done> = tasks
        .iter()
        .filter(|t| t.est_minutes > 0)
        .filter_map(|t| {
            let m = agent_minutes(t)? as f64;
            Some(Done {
                ratio: m / t.est_minutes as f64,
                hours: m / 60.0,
                kind: if t.kind.is_empty() {
                    "untyped".to_string()
                } else {
                    t.kind.clone()
                },
                est: t.est_minutes,
            })
        })
        .collect();
    let all: Vec<&Done> = done.iter().collect();
    let mut kinds: Vec<(&str, usize)> = Vec::new();
    for d in &done {
        match kinds.iter_mut().find(|(k, _)| *k == d.kind) {
            Some((_, n)) => *n += 1,
            None => kinds.push((&d.kind, 1)),
        }
    }
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let by_type = kinds
        .iter()
        .map(|(k, _)| {
            let rows: Vec<&Done> = done.iter().filter(|d| d.kind == *k).collect();
            cell(k, &rows)
        })
        .collect();
    let by_size = SIZES
        .iter()
        .map(|(s, _)| {
            let rows: Vec<&Done> = done.iter().filter(|d| size(d.est) == *s).collect();
            cell(s, &rows)
        })
        .collect();
    let mut waits: Vec<f64> = tasks
        .iter()
        .filter_map(owner_minutes)
        .map(|m| m as f64 / 60.0)
        .collect();
    waits.sort_by(f64::total_cmp);
    let owner_wait = Wait {
        n: waits.len(),
        median_hours: (waits.len() >= MIN_CELL).then(|| round(nearest_rank(&waits, 0.5), 1)),
    };
    Measured {
        overall: cell("all", &all),
        by_type,
        by_size,
        owner_wait,
    }
}

/// A rounded number as the brief prints it: ASCII digits and a dot, no trailing zeros (0.39,
/// 1.2, 2).
pub fn num(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A cell's numbers after its label: "0.39x at p50, 1.2x at p80; median 1.2 h", or "too few (3)".
fn cell_text(c: &Cell) -> String {
    match (c.p50, c.p80, c.median_hours) {
        (Some(p50), Some(p80), Some(h)) => format!(
            "{} finished: {}x at p50, {}x at p80; median {} h",
            c.n,
            num(p50),
            num(p80),
            num(h)
        ),
        _ => format!("too few ({})", c.n),
    }
}

/// The brief's "Estimates, measured on this queue" block, ending in a newline.
pub fn text(m: &Measured) -> String {
    let mut out = String::from("Estimates, measured on this queue (agent time: the first claim to leaving Testing, send-backs, review rounds and QA included, time blocked or paused left out):\n");
    let o = &m.overall;
    match (o.p50, o.p80, o.median_hours) {
        (Some(p50), Some(p80), Some(h)) => {
            out.push_str(&format!(
                "  {} finished: actual is {}x the estimate at p50, {}x at p80; median {} h of agent time\n",
                o.n,
                num(p50),
                num(p80),
                num(h)
            ));
            out.push_str("  By type:\n");
            for c in &m.by_type {
                out.push_str(&format!("    {}: {}\n", c.name, cell_text(c)));
            }
            out.push_str("  By size of the estimate:\n");
            for (c, (_, label)) in m.by_size.iter().zip(SIZES) {
                out.push_str(&format!("    {label}: {}\n", cell_text(c)));
            }
        }
        _ if o.n == 0 => out.push_str(&format!("  {NONE_YET}\n")),
        _ => out.push_str(&format!(
            "  too few ({}) finished tasks with an estimate to measure: estimate by scope\n",
            o.n
        )),
    }
    let wait = match m.owner_wait.median_hours {
        Some(h) => format!("median {} h on this queue", num(h)),
        None => format!("too few ({}) to measure yet", m.owner_wait.n),
    };
    out.push_str(&format!("  {RULE} ({wait}).\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A task of type `kind` estimated at `est` ("" for none), with `log` as its Log lines
    /// (`at from to`, the tool's stage lines), parsed the way the queue reads a file.
    fn task(id: &str, kind: &str, est: &str, log: &[(&str, &str, &str)]) -> Task {
        let lines: Vec<String> = log
            .iter()
            .map(|(at, from, to)| format!("- {at} akbar: stage {from} → {to}"))
            .collect();
        task_lines(id, kind, est, &lines)
    }

    /// A task like `task`'s, with `lines` as its whole Log: stage, block and pause lines.
    fn task_lines(id: &str, kind: &str, est: &str, lines: &[String]) -> Task {
        let est = if est.is_empty() {
            String::new()
        } else {
            format!("est-time: {est}\n")
        };
        let mut text =
            format!("---\nid: {id}\ntype: {kind}\n{est}stage: done\n---\n\n# T\n\n## Log\n");
        for l in lines {
            text.push_str(l);
            text.push('\n');
        }
        crate::queue::parse_task("q", Path::new("/q"), "", Path::new("/q/x.md"), &text, 0)
            .expect("a well-formed test task")
    }

    /// Claimed at 09:00 on 2026-10-01 and into Your check `mins` later, straight through.
    fn built(id: &str, kind: &str, est: &str, mins: i64) -> Task {
        let stamp = |m: i64| format!("2026-10-01 {:02}:{:02}", 9 + m / 60, m % 60);
        task(
            id,
            kind,
            est,
            &[
                (&stamp(0), "ready", "building"),
                (&stamp(mins / 2), "building", "testing"),
                (&stamp(mins), "testing", "your-check"),
            ],
        )
    }

    /// The hand-computed fixture: eight that count, four that don't.
    fn fixture() -> Vec<Task> {
        vec![
            built("1", "code", "1h", 30), // 0.5, S
            built("2", "code", "1h", 60), // 1.0, S
            built("3", "code", "1h", 15), // 0.25, S
            built("4", "code", "2h", 90), // 0.75, M
            // Sent back once: 09:00 → 12:00 = 180 min on 2h, 1.5, M.
            task(
                "5",
                "code",
                "2h",
                &[
                    ("2026-10-01 09:00", "ready", "building"),
                    ("2026-10-01 10:00", "building", "testing"),
                    ("2026-10-01 10:30", "testing", "building"),
                    ("2026-10-01 11:00", "building", "testing"),
                    ("2026-10-01 12:00", "testing", "done"),
                ],
            ),
            built("6", "bug", "1h", 90),   // 1.5, S
            built("7", "bug", "4h", 120),  // 0.5, L
            built("8", "docs", "1h", 120), // 2.0, S
            // Never claimed.
            task(
                "9",
                "code",
                "1h",
                &[("2026-10-01 09:00", "shaping", "ready")],
            ),
            // No estimate.
            built("10", "code", "", 60),
            // Backwards: left Testing before it was claimed.
            task(
                "11",
                "code",
                "1h",
                &[
                    ("2026-10-01 12:00", "ready", "building"),
                    ("2026-10-01 11:00", "testing", "your-check"),
                ],
            ),
            // Still in Building.
            task(
                "12",
                "code",
                "1h",
                &[("2026-10-01 09:00", "ready", "building")],
            ),
        ]
    }

    fn full(name: &str, n: usize, p50: f64, p80: f64, h: f64) -> Cell {
        Cell {
            name: name.into(),
            n,
            p50: Some(p50),
            p80: Some(p80),
            median_hours: Some(h),
        }
    }

    fn few(name: &str, n: usize) -> Cell {
        Cell {
            name: name.into(),
            n,
            p50: None,
            p80: None,
            median_hours: None,
        }
    }

    #[test]
    fn measured_fixture_gives_the_hand_computed_numbers() {
        let m = measure(&fixture());
        // Ratios .25 .5 .5 .75 1 1.5 1.5 2: p50 the 4th, p80 the 7th (⌈6.4⌉).
        // Hours .25 .5 1 1.5 1.5 2 2 3: the 4th.
        assert_eq!(m.overall, full("all", 8, 0.75, 1.5, 1.5));
        assert_eq!(
            m.by_type,
            [
                // .25 .5 .75 1 1.5; hours .25 .5 1 1.5 3.
                full("code", 5, 0.75, 1.0, 1.0),
                few("bug", 2),
                few("docs", 1),
            ]
        );
        assert_eq!(
            m.by_size,
            [
                // 1, 2, 3, 6, 8: .25 .5 1 1.5 2; hours .25 .5 1 1.5 2.
                full("S", 5, 1.0, 1.5, 1.0),
                few("M", 2),
                few("L", 1),
                few("XL", 0),
            ]
        );
    }

    #[test]
    fn measured_send_backs_count_from_the_first_claim_to_the_last_leaving_testing() {
        let t = &fixture()[4];
        assert_eq!(agent_minutes(t), Some(180));
        // Back from Your check and through Testing again: the last leaving counts.
        let again = task(
            "x",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 09:30", "building", "testing"),
                ("2026-10-01 10:00", "testing", "your-check"),
                ("2026-10-01 11:00", "your-check", "building"),
                ("2026-10-01 11:30", "building", "testing"),
                ("2026-10-01 12:15", "testing", "your-check"),
            ],
        );
        assert_eq!(agent_minutes(&again), Some(195));
        // Sent back from Your check and not through Testing again: not finished.
        let back = task(
            "b",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 09:30", "building", "testing"),
                ("2026-10-01 10:00", "testing", "your-check"),
                ("2026-10-01 11:00", "your-check", "building"),
            ],
        );
        assert_eq!(agent_minutes(&back), None);
        // Reopened after Done: not finished either.
        let reopened = task(
            "r",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 09:30", "building", "testing"),
                ("2026-10-01 10:00", "testing", "your-check"),
                ("2026-10-01 11:00", "your-check", "done"),
                ("2026-10-02 09:00", "done", "ready"),
            ],
        );
        assert_eq!(agent_minutes(&reopened), None);
    }

    #[test]
    fn measured_skips_unclaimed_unestimated_backwards_and_unfinished_tasks() {
        let f = fixture();
        assert_eq!(agent_minutes(&f[8]), None, "never claimed");
        assert_eq!(agent_minutes(&f[10]), None, "backwards");
        assert_eq!(agent_minutes(&f[11]), None, "still building");
        // The one without an estimate has a span but isn't counted.
        assert_eq!(agent_minutes(&f[9]), Some(60));
        assert_eq!(measure(&f[8..]).overall, few("all", 0));
        // A zero span (same minute) is skipped too.
        let zero = task(
            "z",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 09:00", "testing", "done"),
            ],
        );
        assert_eq!(agent_minutes(&zero), None);
        // Building → Done without Testing never left Testing.
        let skipped = task(
            "s",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 10:00", "building", "done"),
            ],
        );
        assert_eq!(agent_minutes(&skipped), None);
    }

    #[test]
    fn measured_owner_wait_is_your_check_to_done_and_kept_apart() {
        let t = task(
            "w",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 09:30", "building", "testing"),
                ("2026-10-01 10:00", "testing", "your-check"),
                ("2026-10-01 11:00", "your-check", "testing"),
                ("2026-10-01 11:30", "testing", "your-check"),
                ("2026-10-01 14:00", "your-check", "done"),
            ],
        );
        // The last entry into Your check → Done: 11:30 → 14:00.
        assert_eq!(owner_minutes(&t), Some(150));
        // Agent time stops at the last leaving Testing, not at Done.
        assert_eq!(agent_minutes(&t), Some(150));
        assert_eq!(owner_minutes(&fixture()[0]), None, "still in Your check");
        assert_eq!(owner_minutes(&fixture()[4]), None, "never in Your check");
        // Sent back from Your check, rebuilt and put in Done from Testing: the rebuild isn't the
        // owner's wait.
        let over = task(
            "o",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 09:30", "building", "testing"),
                ("2026-10-01 10:00", "testing", "your-check"),
                ("2026-10-01 10:30", "your-check", "building"),
                ("2026-10-01 11:00", "building", "testing"),
                ("2026-10-01 12:00", "testing", "done"),
            ],
        );
        assert_eq!(owner_minutes(&over), None);
        assert_eq!(agent_minutes(&over), Some(180));
        // A task with no type is measured as untyped, never as an empty name.
        let mut t = built("u", "code", "1h", 30);
        t.kind = String::new();
        assert_eq!(measure(&[t]).by_type[0].name, "untyped");
        let waits: Vec<Task> = [60, 120, 180, 240, 600]
            .iter()
            .map(|m| {
                let end = format!("2026-10-02 {:02}:{:02}", 8 + m / 60, m % 60);
                task(
                    "w",
                    "code",
                    "",
                    &[
                        ("2026-10-02 08:00", "testing", "your-check"),
                        (&end, "your-check", "done"),
                    ],
                )
            })
            .collect();
        let m = measure(&waits);
        assert_eq!(
            m.owner_wait,
            Wait {
                n: 5,
                median_hours: Some(3.0)
            }
        );
        // Unestimated, so none of them counts for agent time.
        assert_eq!(m.overall.n, 0);
        assert_eq!(measure(&waits[..4]).owner_wait.median_hours, None);
    }

    #[test]
    fn measured_owner_wait_counts_an_override_from_building_but_agent_time_does_not() {
        // Moved into Your check by the owner's override, never through Testing.
        let t = task(
            "v",
            "code",
            "1h",
            &[
                ("2026-10-01 09:00", "ready", "building"),
                ("2026-10-01 10:00", "building", "your-check"),
                ("2026-10-01 12:30", "your-check", "done"),
            ],
        );
        assert_eq!(owner_minutes(&t), Some(150));
        assert_eq!(agent_minutes(&t), None);
        let m = measure(&[t]);
        assert_eq!((m.overall.n, m.owner_wait.n), (0, 1));
    }

    #[test]
    fn measured_agent_time_leaves_out_blocked_and_paused_stretches() {
        let stage = |at: &str, from: &str, to: &str| format!("- {at} akbar: stage {from} → {to}");
        // 09:00 → 12:00 is 180 min; blocked 09:30-10:30 and paused 10:00-11:00 overlap, so 90 min
        // stopped (each minute once) and 90 worked.
        let t = task_lines(
            "b",
            "code",
            "1h",
            &[
                stage("2026-10-01 09:00", "ready", "building"),
                "- 2026-10-01 09:30 akbar: blocked on question: Q1".into(),
                "- 2026-10-01 10:00 owner: paused (lunch)".into(),
                "- 2026-10-01 10:30 owner: unblocked: Q1 answered".into(),
                "- 2026-10-01 11:00 owner: resumed after 1 h".into(),
                stage("2026-10-01 11:30", "building", "testing"),
                stage("2026-10-01 12:00", "testing", "your-check"),
            ],
        );
        assert_eq!(t.block_history.len(), 1, "{:?}", t.block_history);
        assert_eq!(t.pause_history.len(), 1, "{:?}", t.pause_history);
        assert_eq!(agent_minutes(&t), Some(90));
        assert_eq!(measure(&[t]).overall.n, 1);
        // A block from before the claim counts only from the claim (09:00-09:15 of it), and one
        // still open at the end runs to the end (09:30-10:00): 60 - 15 - 30 = 15.
        let t = task_lines(
            "c",
            "code",
            "1h",
            &[
                "- 2026-10-01 08:00 akbar: blocked on outside: Apple review".into(),
                stage("2026-10-01 09:00", "ready", "building"),
                "- 2026-10-01 09:15 owner: unblocked: approved".into(),
                "- 2026-10-01 09:30 akbar: blocked on you: which colour".into(),
                stage("2026-10-01 10:00", "testing", "your-check"),
            ],
        );
        assert_eq!(agent_minutes(&t), Some(15));
        // A block or pause after leaving Testing isn't agent time either way.
        let t = task_lines(
            "d",
            "code",
            "1h",
            &[
                stage("2026-10-01 09:00", "ready", "building"),
                stage("2026-10-01 10:00", "testing", "your-check"),
                "- 2026-10-01 10:30 owner: paused".into(),
            ],
        );
        assert_eq!(agent_minutes(&t), Some(60));
        // Blocked in Testing and unblocked only after the move to Your check (the owner's answer
        // after their override): the block counts to the end (09:30-10:00), not to 11:00.
        let t = task_lines(
            "f",
            "code",
            "1h",
            &[
                stage("2026-10-01 09:00", "ready", "building"),
                stage("2026-10-01 09:20", "building", "testing"),
                "- 2026-10-01 09:30 akbar: blocked on you: which colour".into(),
                stage("2026-10-01 10:00", "testing", "your-check"),
                "- 2026-10-01 11:00 owner: unblocked: blue".into(),
            ],
        );
        assert_eq!(t.block_history.len(), 1, "{:?}", t.block_history);
        assert_eq!(agent_minutes(&t), Some(30));
        // Blocked from the claim to the end: nothing worked, so it's skipped.
        let t = task_lines(
            "e",
            "code",
            "1h",
            &[
                stage("2026-10-01 09:00", "ready", "building"),
                "- 2026-10-01 09:00 akbar: blocked on tasks: 0014".into(),
                stage("2026-10-01 10:00", "testing", "your-check"),
            ],
        );
        assert_eq!(agent_minutes(&t), None);
    }

    #[test]
    fn measured_json_and_text_carry_the_same_rounded_numbers() {
        // Estimated 3h, took 70-100 min: ratios .3889 .4167 .4722 .5278 .5556, none exact.
        let tasks: Vec<Task> = [70, 75, 85, 95, 100]
            .iter()
            .map(|m| built("r", "code", "3h", *m))
            .collect();
        let m = measure(&tasks);
        // p50 the 3rd (85/180), p80 the 4th (95/180), median 85 min.
        assert_eq!(m.overall, full("all", 5, 0.47, 0.53, 1.4));
        let v = serde_json::json!(m);
        assert_eq!(v["overall"]["p50"], 0.47);
        assert_eq!(v["overall"]["p80"], 0.53);
        assert_eq!(v["overall"]["medianHours"], 1.4);
        assert_eq!(v["bySize"][1]["p50"], 0.47);
        let s = text(&m);
        assert!(
            s.contains("  5 finished: actual is 0.47x the estimate at p50, 0.53x at p80; median 1.4 h of agent time\n"),
            "{s}"
        );
        assert!(
            s.contains("    M (up to 3h): 5 finished: 0.47x at p50, 0.53x at p80; median 1.4 h\n"),
            "{s}"
        );
    }

    #[test]
    fn measured_types_with_the_same_count_come_by_name() {
        // zeta is seen first, with as many tasks as alpha; most first, then by name.
        let tasks = [
            built("1", "zeta", "1h", 30),
            built("2", "alpha", "1h", 30),
            built("3", "zeta", "1h", 30),
            built("4", "alpha", "1h", 30),
            built("5", "beta", "1h", 30),
            built("6", "beta", "1h", 30),
            built("7", "beta", "1h", 30),
        ];
        let names: Vec<String> = measure(&tasks)
            .by_type
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["beta", "alpha", "zeta"]);
    }

    #[test]
    fn measured_sizes_split_at_1h_3h_and_8h() {
        assert_eq!(size(1), "S");
        assert_eq!(size(60), "S");
        assert_eq!(size(61), "M");
        assert_eq!(size(180), "M");
        assert_eq!(size(181), "L");
        assert_eq!(size(480), "L");
        assert_eq!(size(481), "XL");
    }

    #[test]
    fn measured_nearest_rank_on_small_and_one_element_sets() {
        assert_eq!(nearest_rank(&[3.0], 0.5), 3.0);
        assert_eq!(nearest_rank(&[3.0], 0.8), 3.0);
        assert_eq!(nearest_rank(&[1.0, 2.0], 0.5), 1.0);
        assert_eq!(nearest_rank(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.8), 4.0);
        assert_eq!(nearest_rank(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.5), 3.0);
    }

    #[test]
    fn measured_text_prints_the_block_with_too_few_cells_and_the_rule() {
        let s = text(&measure(&fixture()));
        assert!(s.starts_with("Estimates, measured on this queue"), "{s}");
        assert!(
            s.contains("  8 finished: actual is 0.75x the estimate at p50, 1.5x at p80; median 1.5 h of agent time\n"),
            "{s}"
        );
        assert!(
            s.contains("    code: 5 finished: 0.75x at p50, 1x at p80; median 1 h\n"),
            "{s}"
        );
        assert!(s.contains("    bug: too few (2)\n"), "{s}");
        assert!(
            s.contains("    S (up to 1h): 5 finished: 1x at p50, 1.5x at p80; median 1 h\n"),
            "{s}"
        );
        assert!(s.contains("    XL (over 8h): too few (0)\n"), "{s}");
        assert!(s.contains("--time is agent time: the first claim to leaving Testing, review rounds and QA included, time blocked or paused left out; --test-time is the owner's check only (QA is in --time); the owner's wait in Your check is neither (too few (0) to measure yet).\n"), "{s}");
        assert!(!s.contains(NONE_YET));
    }

    #[test]
    fn measured_text_with_nothing_finished_says_estimate_by_scope() {
        let s = text(&measure(&[]));
        assert!(s.contains(&format!("  {NONE_YET}\n")), "{s}");
        assert!(s.contains(RULE), "{s}");
        assert!(!s.contains("By type"), "{s}");
        let s = text(&measure(&fixture()[..3]));
        assert!(
            s.contains(
                "  too few (3) finished tasks with an estimate to measure: estimate by scope\n"
            ),
            "{s}"
        );
    }

    #[test]
    fn measured_numbers_print_with_ascii_digits_and_no_trailing_zeros() {
        assert_eq!(num(0.39), "0.39");
        assert_eq!(num(1.2), "1.2");
        assert_eq!(num(2.0), "2");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(12.5), "12.5");
    }

    #[test]
    fn measured_500_tasks_take_under_10_ms() {
        let tasks: Vec<Task> = (0..500)
            .map(|i| {
                let kind = ["code", "bug", "docs", "chore"][i % 4];
                let est = ["30m", "2h", "6h", "2d"][i % 4];
                built(&i.to_string(), kind, est, 15 + (i as i64 % 400))
            })
            .collect();
        // Best of three: the machine may be busy.
        let best = (0..3)
            .map(|_| {
                let t = std::time::Instant::now();
                let m = measure(&tasks);
                assert_eq!(m.overall.n, 500);
                t.elapsed()
            })
            .min()
            .unwrap();
        println!("measured 500 tasks in {best:?}");
        assert!(best < std::time::Duration::from_millis(10), "{best:?}");
    }
}
