//! Jalali (Solar Hijri) dates for the command line in Persian (mu-152): the dates `muster` prints
//! for a person, shown in the Persian calendar. Storage keeps Gregorian `YYYY-MM-DD`: these only
//! convert at the edge. A port of mu-151's `src/jalali.ts`, the conversions only (the window also
//! reads typed dates; the command line doesn't).
//!
//! Ported from @dastafzar/calendar (~/playground/persianUtils/calendar/main/packages/calendar/src/
//! conversion.ts, with year-table.ts inlined), MIT licence, Copyright (c) 2026 persianUtils
//! contributors.
//!
//! Which day a year starts on (the source's decision 0001):
//! - 1200–1600 AP: an astronomical table, the March equinox judged against true noon at 52.5° E.
//! - Outside: the 33-year rule with ICU4X's correction list, the rule ICU, Safari, Chrome and Node
//!   `Intl` use. It agrees with the table on every year start 1200–1601, so the seams are smooth.
//!
//! Every day is an integer "epoch day": days since 1970-01-01 (proleptic Gregorian). Conversions go
//! through it, never through the clock, so they don't depend on the time zone.

use std::sync::OnceLock;

/// A calendar day: `year`, `month` (1–12) and `day`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateFields {
    pub year: i64,
    pub month: i64,
    pub day: i64,
}

const MIN_YEAR: i64 = 1;
const MAX_YEAR: i64 = 9999;

// The astronomical table (the source's year-table.ts, generated from its data/equinox-table.json).
const TABLE_FIRST_YEAR: i64 = 1200;
const TABLE_LAST_YEAR: i64 = 1600;
/// Days from 1970-01-01 to 1 Farvardin 1200 (1821-03-21).
const TABLE_FIRST_NOWRUZ: i64 = -54342;
/// One bit per year 1200–1600, most significant first: 1 = leap (30 Esfand).
const TABLE_LEAP_BITS: &[u8] =
    b"44222222221111111108888888844444444222222221111111108888888844444444222222211111111108888888844444440";

/// ICU4X `NON_LEAP_CORRECTION` (utils/calendrical_calculations/src/persian.rs, commit 203a0561),
/// from 1601 on: years the plain 33-year rule calls leap that aren't; the next year is leap
/// instead. Earlier entries (1502) fall inside the table.
const CORRECTIONS: [i64; 77] = [
    1601, 1634, 1667, 1700, 1733, 1766, 1799, 1832, 1865, 1898, 1931, 1964, 1997, 2030, 2059, 2063,
    2096, 2129, 2158, 2162, 2191, 2195, 2224, 2228, 2257, 2261, 2290, 2294, 2323, 2327, 2356, 2360,
    2389, 2393, 2422, 2426, 2455, 2459, 2488, 2492, 2521, 2525, 2554, 2558, 2587, 2591, 2620, 2624,
    2653, 2657, 2686, 2690, 2719, 2723, 2748, 2752, 2756, 2781, 2785, 2789, 2818, 2822, 2847, 2851,
    2855, 2880, 2884, 2888, 2913, 2917, 2921, 2946, 2950, 2954, 2979, 2983, 2987,
];

/// ICU4X's `PERSIAN_EPOCH - 1` as an epoch day: the Persian epoch is Julian 622-03-19, which is
/// 622-03-22 proleptic Gregorian, epoch day −492267.
const RULE_BASE: i64 = -492268;

/// The Persian months, as Persian writes them.
const MONTHS_FA: [&str; 12] = [
    "فروردین",
    "اردیبهشت",
    "خرداد",
    "تیر",
    "مرداد",
    "شهریور",
    "مهر",
    "آبان",
    "آذر",
    "دی",
    "بهمن",
    "اسفند",
];

/// 1 Farvardin by the corrected 33-year rule (ICU4X `fixed_from_fast_persian`).
fn rule_nowruz(year: i64) -> i64 {
    let start = RULE_BASE + 365 * (year - 1) + (8 * year + 21).div_euclid(33);
    if CORRECTIONS.contains(&(year - 1)) {
        start - 1
    } else {
        start
    }
}

/// Epoch day of 1 Farvardin `TABLE_FIRST_YEAR` … `TABLE_LAST_YEAR + 1`, the table built on first use.
fn table_nowruz(year: i64) -> i64 {
    static STARTS: OnceLock<Vec<i64>> = OnceLock::new();
    let starts = STARTS.get_or_init(|| {
        let count = TABLE_LAST_YEAR - TABLE_FIRST_YEAR + 2;
        let mut starts = vec![TABLE_FIRST_NOWRUZ];
        for i in 1..count {
            let bit = usize::try_from(i - 1)
                .ok()
                .and_then(|k| TABLE_LEAP_BITS.get(k >> 2).map(|nibble| (nibble, k & 3)))
                .and_then(|(nibble, at)| {
                    char::from(*nibble)
                        .to_digit(16)
                        .map(|n| (n >> (3 - at)) & 1)
                })
                .unwrap_or(0);
            let last = starts.last().copied().unwrap_or(TABLE_FIRST_NOWRUZ);
            starts.push(last + 365 + i64::from(bit));
        }
        starts
    });
    usize::try_from(year - TABLE_FIRST_YEAR)
        .ok()
        .and_then(|i| starts.get(i).copied())
        .unwrap_or_else(|| rule_nowruz(year))
}

/// Epoch day of 1 Farvardin of `year` (no range check).
fn nowruz(year: i64) -> i64 {
    if (TABLE_FIRST_YEAR..=TABLE_LAST_YEAR + 1).contains(&year) {
        table_nowruz(year)
    } else {
        rule_nowruz(year)
    }
}

fn check_year(year: i64) -> Result<(), String> {
    if (MIN_YEAR..=MAX_YEAR).contains(&year) {
        Ok(())
    } else {
        Err(format!(
            "Jalali year {year} is outside {MIN_YEAR}–{MAX_YEAR} AP, the years this converts"
        ))
    }
}

/// Whether `year` is a leap year (Esfand has 30 days).
pub fn is_leap_year(year: i64) -> Result<bool, String> {
    check_year(year)?;
    Ok(nowruz(year + 1) - nowruz(year) == 366)
}

/// 31 for months 1–6, 30 for 7–11, 29 or 30 for Esfand (12).
pub fn days_in_month(year: i64, month: i64) -> Result<i64, String> {
    check_year(year)?;
    match month {
        1..=6 => Ok(31),
        7..=11 => Ok(30),
        12 => Ok(nowruz(year + 1) - nowruz(year) - 336),
        _ => Err(format!("Jalali month {month} is not 1–12")),
    }
}

/// Whether the fields name a real Jalali day in 1–9999 AP.
pub fn is_valid(year: i64, month: i64, day: i64) -> bool {
    days_in_month(year, month).is_ok_and(|n| (1..=n).contains(&day))
}

/// Days since 1970-01-01 of a Jalali date; an error for an impossible one.
pub fn jalali_to_epoch_day(year: i64, month: i64, day: i64) -> Result<i64, String> {
    let length = days_in_month(year, month)?;
    if !(1..=length).contains(&day) {
        return Err(format!(
            "{year}/{month}/{day} is not a Jalali date: month {month} of {year} has {length} days"
        ));
    }
    let before = if month <= 7 {
        31 * (month - 1)
    } else {
        186 + 30 * (month - 7)
    };
    Ok(nowruz(year) + before + day - 1)
}

/// The Jalali date of an epoch day (days since 1970-01-01).
pub fn epoch_day_to_jalali(epoch_day: i64) -> Result<DateFields, String> {
    if epoch_day < nowruz(MIN_YEAR) || epoch_day >= nowruz(MAX_YEAR + 1) {
        return Err(format!(
            "epoch day {epoch_day} is outside {MIN_YEAR}–{MAX_YEAR} AP, the years this converts"
        ));
    }
    // 365.2422 days a year, as a first guess the loops below correct.
    let mut year = ((epoch_day - RULE_BASE) * 10_000)
        .div_euclid(3_652_422)
        .saturating_add(1)
        .clamp(MIN_YEAR, MAX_YEAR);
    while nowruz(year) > epoch_day {
        year -= 1;
    }
    while nowruz(year + 1) <= epoch_day {
        year += 1;
    }
    let day_of_year = epoch_day - nowruz(year);
    Ok(if day_of_year < 186 {
        DateFields {
            year,
            month: day_of_year / 31 + 1,
            day: day_of_year % 31 + 1,
        }
    } else {
        let rest = day_of_year - 186;
        DateFields {
            year,
            month: rest / 30 + 7,
            day: rest % 30 + 1,
        }
    })
}

fn is_gregorian_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days since 1970-01-01 of a proleptic Gregorian date; an error for an impossible one.
pub fn gregorian_to_epoch_day(year: i64, month: i64, day: i64) -> Result<i64, String> {
    if !(1..=12).contains(&month) {
        return Err(format!("Gregorian month {month} is not 1–12"));
    }
    if !(-1_000_000..=1_000_000).contains(&year) {
        return Err(format!("Gregorian year {year} is out of range"));
    }
    let length = if month == 2 {
        if is_gregorian_leap_year(year) {
            29
        } else {
            28
        }
    } else {
        30 + ((month + (month >> 3)) & 1)
    };
    if !(1..=length).contains(&day) {
        return Err(format!(
            "{year}-{month}-{day} is not a Gregorian date: the month has {length} days"
        ));
    }
    // Howard Hinnant's days_from_civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Ok(era * 146_097 + day_of_era - 719_468)
}

/// The proleptic Gregorian date of an epoch day.
pub fn epoch_day_to_gregorian(epoch_day: i64) -> DateFields {
    // Howard Hinnant's civil_from_days.
    let z = epoch_day + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    DateFields {
        year: year_of_era + era * 400 + i64::from(month <= 2),
        month,
        day,
    }
}

/// Gregorian → Jalali; an error for an impossible date.
pub fn gregorian_to_jalali(year: i64, month: i64, day: i64) -> Result<DateFields, String> {
    epoch_day_to_jalali(gregorian_to_epoch_day(year, month, day)?)
}

/// Jalali → Gregorian; an error for an impossible date (30 Esfand of a common year).
pub fn jalali_to_gregorian(year: i64, month: i64, day: i64) -> Result<DateFields, String> {
    Ok(epoch_day_to_gregorian(jalali_to_epoch_day(
        year, month, day,
    )?))
}

/// `digits` ASCII digits exactly, as a number.
fn number(s: &str, digits: usize) -> Option<i64> {
    if s.len() == digits && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

/// A stamp Muster writes (`YYYY-MM-DD HH:MM`, or a day `YYYY-MM-DD`) as the command line shows it
/// in `locale`: as it is, except in Persian, where the day is Jalali, `22 مهر 1405 23:40`, in
/// Latin digits (mu-152: what's printed may be copied into a command). Anything else, as it is.
pub fn stamp_in(locale: &str, stamp: &str) -> String {
    if locale != "fa" {
        return stamp.to_string();
    }
    let (day, time) = match stamp.split_once(' ') {
        Some((d, t)) => (d, Some(t)),
        None => (stamp, None),
    };
    let time_ok = time.is_none_or(|t| {
        t.split_once(':').is_some_and(|(h, m)| {
            number(h, 2).is_some_and(|h| h < 24) && number(m, 2).is_some_and(|m| m < 60)
        })
    });
    let mut parts = day.split('-');
    let fields = (
        parts.next().and_then(|y| number(y, 4)),
        parts.next().and_then(|m| number(m, 2)),
        parts.next().and_then(|d| number(d, 2)),
        parts.next(),
    );
    let (Some(y), Some(m), Some(d), None) = fields else {
        return stamp.to_string();
    };
    let Some(j) = time_ok.then(|| gregorian_to_jalali(y, m, d).ok()).flatten() else {
        return stamp.to_string();
    };
    let Some(month) = usize::try_from(j.month - 1)
        .ok()
        .and_then(|i| MONTHS_FA.get(i))
    else {
        return stamp.to_string();
    };
    match time {
        Some(t) => format!("{} {month} {} {t}", j.day, j.year),
        None => format!("{} {month} {}", j.day, j.year),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "1404/07/20" or "2025-10-12" as its fields.
    fn fields(s: &str) -> DateFields {
        let n: Vec<i64> = s.split(['/', '-']).map(|p| p.parse().unwrap()).collect();
        DateFields {
            year: n[0],
            month: n[1],
            day: n[2],
        }
    }

    /// mu-151's shared cases, the file `jalali.test.ts` reads too, so both ports answer the same
    /// dates: `known` is [Jalali, Gregorian], the same day both ways; `impossible` is no day.
    #[derive(serde::Deserialize)]
    struct Cases {
        known: Vec<(String, String)>,
        impossible: Vec<String>,
    }

    fn cases() -> Cases {
        serde_json::from_str(include_str!("../../src/jalali-cases.json"))
            .expect("jalali-cases.json parses")
    }

    #[test]
    fn the_shared_cases_convert_both_ways() {
        let known = cases().known;
        assert!(known.len() >= 9, "the shared file lost cases");
        for (jalali, gregorian) in &known {
            let (j, g) = (fields(jalali), fields(gregorian));
            assert_eq!(
                jalali_to_gregorian(j.year, j.month, j.day),
                Ok(g),
                "{jalali}"
            );
            assert_eq!(
                gregorian_to_jalali(g.year, g.month, g.day),
                Ok(j),
                "{gregorian}"
            );
        }
    }

    #[test]
    fn an_impossible_date_is_no_day() {
        let impossible = cases().impossible;
        assert!(impossible.len() >= 9, "the shared file lost cases");
        for jalali in &impossible {
            let j = fields(jalali);
            assert!(!is_valid(j.year, j.month, j.day), "{jalali}");
            assert!(
                jalali_to_epoch_day(j.year, j.month, j.day).is_err(),
                "{jalali}"
            );
            assert!(
                jalali_to_gregorian(j.year, j.month, j.day).is_err(),
                "{jalali}"
            );
        }
        assert!(gregorian_to_epoch_day(2025, 2, 29).is_err());
        assert!(gregorian_to_epoch_day(2025, 13, 1).is_err());
        assert!(gregorian_to_epoch_day(2025, 4, 31).is_err());
        assert!(epoch_day_to_jalali(nowruz(MIN_YEAR) - 1).is_err());
        assert!(epoch_day_to_jalali(nowruz(MAX_YEAR + 1)).is_err());
        assert!(epoch_day_to_jalali(nowruz(MAX_YEAR + 1) - 1).is_ok());
    }

    #[test]
    fn months_and_leap_years() {
        let lengths: Vec<i64> = [1, 6, 7, 11]
            .iter()
            .map(|m| days_in_month(1404, *m).unwrap())
            .collect();
        assert_eq!(lengths, [31, 31, 30, 30]);
        assert_eq!(days_in_month(1403, 12), Ok(30));
        assert_eq!(days_in_month(1404, 12), Ok(29));
        assert!(days_in_month(1404, 13).is_err());
        assert_eq!(is_leap_year(1403), Ok(true));
        assert_eq!(is_leap_year(1404), Ok(false));
        assert!(is_leap_year(0).is_err() && is_leap_year(10_000).is_err());
        assert!(is_valid(1403, 12, 30) && !is_valid(1404, 12, 30));
    }

    #[test]
    fn counts_epoch_days_from_1970() {
        assert_eq!(gregorian_to_epoch_day(1970, 1, 1), Ok(0));
        assert_eq!(epoch_day_to_gregorian(-1), fields("1969-12-31"));
        assert_eq!(epoch_day_to_jalali(0), Ok(fields("1348/10/11")));
        assert_eq!(epoch_day_to_gregorian(-492_267), fields("622-03-22"));
    }

    /// 1 Farvardin of each year 1369–1420, as ICU's Persian calendar (`Intl`, `en-u-ca-persian`)
    /// has it: an answer from outside this port.
    const NOWRUZ: [(i64, &str); 52] = [
        (1369, "1990-03-21"),
        (1370, "1991-03-21"),
        (1371, "1992-03-21"),
        (1372, "1993-03-21"),
        (1373, "1994-03-21"),
        (1374, "1995-03-21"),
        (1375, "1996-03-20"),
        (1376, "1997-03-21"),
        (1377, "1998-03-21"),
        (1378, "1999-03-21"),
        (1379, "2000-03-20"),
        (1380, "2001-03-21"),
        (1381, "2002-03-21"),
        (1382, "2003-03-21"),
        (1383, "2004-03-20"),
        (1384, "2005-03-21"),
        (1385, "2006-03-21"),
        (1386, "2007-03-21"),
        (1387, "2008-03-20"),
        (1388, "2009-03-21"),
        (1389, "2010-03-21"),
        (1390, "2011-03-21"),
        (1391, "2012-03-20"),
        (1392, "2013-03-21"),
        (1393, "2014-03-21"),
        (1394, "2015-03-21"),
        (1395, "2016-03-20"),
        (1396, "2017-03-21"),
        (1397, "2018-03-21"),
        (1398, "2019-03-21"),
        (1399, "2020-03-20"),
        (1400, "2021-03-21"),
        (1401, "2022-03-21"),
        (1402, "2023-03-21"),
        (1403, "2024-03-20"),
        (1404, "2025-03-21"),
        (1405, "2026-03-21"),
        (1406, "2027-03-21"),
        (1407, "2028-03-20"),
        (1408, "2029-03-20"),
        (1409, "2030-03-21"),
        (1410, "2031-03-21"),
        (1411, "2032-03-20"),
        (1412, "2033-03-20"),
        (1413, "2034-03-21"),
        (1414, "2035-03-21"),
        (1415, "2036-03-20"),
        (1416, "2037-03-20"),
        (1417, "2038-03-21"),
        (1418, "2039-03-21"),
        (1419, "2040-03-20"),
        (1420, "2041-03-20"),
    ];

    /// Every day 1990–2040: Gregorian → Jalali → Gregorian gives the day back, each day's Jalali
    /// date follows the one before it by a day (month and year lengths as the calendar has them),
    /// and each 1 Farvardin is the day ICU says.
    #[test]
    fn every_day_1990_to_2040_round_trips_and_follows_the_last() {
        let first = gregorian_to_epoch_day(1990, 1, 1).unwrap();
        let last = gregorian_to_epoch_day(2040, 12, 31).unwrap();
        let nowruz: std::collections::HashMap<i64, DateFields> =
            NOWRUZ.iter().map(|(y, g)| (*y, fields(g))).collect();
        let mut before = epoch_day_to_jalali(first - 1).unwrap();
        let mut new_years = 0;
        for day in first..=last {
            let g = epoch_day_to_gregorian(day);
            let j = gregorian_to_jalali(g.year, g.month, g.day).unwrap();
            assert_eq!(jalali_to_gregorian(j.year, j.month, j.day), Ok(g), "{g:?}");
            assert_eq!(jalali_to_epoch_day(j.year, j.month, j.day), Ok(day));
            let next = if before.day < days_in_month(before.year, before.month).unwrap() {
                DateFields {
                    day: before.day + 1,
                    ..before
                }
            } else if before.month < 12 {
                DateFields {
                    month: before.month + 1,
                    day: 1,
                    ..before
                }
            } else {
                DateFields {
                    year: before.year + 1,
                    month: 1,
                    day: 1,
                }
            };
            assert_eq!(j, next, "{g:?}");
            if j.month == 1 && j.day == 1 {
                assert_eq!(nowruz.get(&j.year), Some(&g), "Nowruz {}", j.year);
                new_years += 1;
            }
            before = j;
        }
        assert_eq!(new_years, 51);
        assert_eq!(last - first + 1, 18_628);
        assert_eq!(gregorian_to_jalali(2041, 3, 20), Ok(fields("1420/01/01")));
    }

    /// mu-152: a stamp in Persian is its Jalali day with the month's name, in Latin digits.
    #[test]
    fn a_stamp_reads_jalali_in_persian_and_as_it_is_otherwise() {
        assert_eq!(stamp_in("fa", "2026-10-14 23:40"), "22 مهر 1405 23:40");
        assert_eq!(stamp_in("fa", "2026-10-14"), "22 مهر 1405");
        assert_eq!(stamp_in("fa", "2025-03-21 00:00"), "1 فروردین 1404 00:00");
        assert_eq!(stamp_in("fa", "2025-03-20"), "30 اسفند 1403");
        assert_eq!(stamp_in("fa", "2026-01-21"), "1 بهمن 1404");
        assert_eq!(stamp_in("en", "2026-10-14 23:40"), "2026-10-14 23:40");
        assert_eq!(stamp_in("de", "2026-10-14"), "2026-10-14");
        for odd in [
            "",
            "soon",
            "2026-10-14T23:40",
            "2026-10-14 23:40:05",
            "2026-10-14 24:00",
            "2026-10-14 23:60",
            "2026-10-14 ",
            "2026-02-30",
            "2026-13-01",
            "2026-1-14",
            "26-10-14",
            "2026-10-14-1",
            "۲۰۲۶-۱۰-۱۴",
            "+026-10-14",
        ] {
            assert_eq!(stamp_in("fa", odd), odd, "{odd:?}");
        }
    }
}
