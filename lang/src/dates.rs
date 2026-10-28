//! Dates of the timeline and frieze modes (F-PARSE-11, F-PARSE-12), without dependency or `Date`:
//! proleptic Gregorian calendar, integer computations.
//!
//! - Timeline: `YYYY`, `YYYY-MM`, `YYYY-MM-DD` → days since 1970-01-01. **`end` is included**: a
//!   period ends on the day after the last day it covers.
//! - Frieze: a year (negative = BC, **no year 0**), optionally followed by `-MM` or `-MM-DD` →
//!   **decimal astronomical** year (−1 → 0, 1 BC and 1 AD one year apart). `end` is a **bound**
//!   (nothing added).

use crate::tr;

/// Days since 1970-01-01 of day (y, m, d) (the "days from civil" algorithm).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month, day) of a number of days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Day of the week (0 = Monday).
pub fn weekday(days: i64) -> u32 {
    ((days + 3).rem_euclid(7)) as u32
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// Timeline date → days. `is_end`: end included (the day after the last day covered).
pub fn parse_timeline(s: &str, is_end: bool) -> Result<f64, String> {
    let bad = || {
        tr!(
            "invalid date: « {s} » (expected YYYY, YYYY-MM or YYYY-MM-DD)",
            "date invalide : « {s} » (attendu AAAA, AAAA-MM ou AAAA-MM-JJ)"
        )
    };
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.is_empty()
        || parts.len() > 3
        || parts[0].len() != 4
        || !parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    let m: u32 = match parts.get(1) {
        Some(p) => p.parse().map_err(|_| bad())?,
        None => 1,
    };
    if !(1..=12).contains(&m) {
        return Err(bad());
    }
    let d: u32 = match parts.get(2) {
        Some(p) => p.parse().map_err(|_| bad())?,
        None => 1,
    };
    if d < 1 || d > days_in_month(y, m) {
        return Err(bad());
    }
    let t = days_from_civil(y, m, d);
    Ok(if !is_end {
        t as f64
    } else if parts.len() == 3 {
        (t + 1) as f64
    } else if parts.len() == 2 {
        (t + i64::from(days_in_month(y, m))) as f64
    } else {
        days_from_civil(y + 1, 1, 1) as f64
    })
}

/// Frieze date → decimal astronomical year (`year + (month − 1)/12 + (day − 1)/365`, as the
/// prototype). Refuses year 0. `is_end`: end included (bound of the `to` options).
pub fn parse_frise(s: &str, is_end: bool) -> Result<f64, String> {
    let t = s.trim();
    let bad = || {
        tr!(
            "invalid date: « {s} » (expected a year, optionally followed by -MM or -MM-DD)",
            "date invalide : « {s} » (attendu une année, éventuellement suivie de -MM ou -MM-JJ)"
        )
    };
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let parts: Vec<&str> = rest.split('-').collect();
    if parts.is_empty()
        || parts.len() > 3
        || parts[0].len() > 5
        || !parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    if y == 0 {
        return Err(tr!(
            "« {s} »: there is no year 0 (1 BC = -1, then 1)",
            "« {s} » : il n'y a pas d'an 0 (1 av. J.-C. = -1, puis 1)"
        ));
    }
    let year = if neg { -y } else { y };
    let astro = if year < 0 { year + 1 } else { year };
    let m: u32 = match parts.get(1) {
        Some(p) => p.parse().map_err(|_| bad())?,
        None => 1,
    };
    if !(1..=12).contains(&m) {
        return Err(bad());
    }
    let d: u32 = match parts.get(2) {
        Some(p) => p.parse().map_err(|_| bad())?,
        None => 1,
    };
    if d < 1 || d > days_in_month(astro, m) {
        return Err(bad());
    }
    let mut v = astro as f64 + f64::from(m - 1) / 12.0 + f64::from(d - 1) / 365.0;
    if is_end {
        v += if parts.len() == 3 {
            1.0 / 365.0
        } else if parts.len() == 2 {
            1.0 / 12.0
        } else {
            1.0
        };
    }
    Ok(v)
}

/// Historical year (without year 0) of a whole astronomical year.
pub fn to_historical(a: i64) -> i64 {
    if a <= 0 { a - 1 } else { a }
}

/// Astronomical year of a historical year.
pub fn to_astronomical(y: i64) -> i64 {
    if y < 0 { y + 1 } else { y }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 10, 15), 20_741);
        assert_eq!(civil_from_days(20_741), (2026, 10, 15));
        assert_eq!(weekday(20_741), 3); // jeudi
        for z in [-800_000, -1, 0, 59, 365, 20_741, 1_000_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn timeline_end_inclusive() {
        assert_eq!(
            parse_timeline("2027-03", true).unwrap() - parse_timeline("2027-03", false).unwrap(),
            31.0
        );
        assert_eq!(
            parse_timeline("2027-03-15", true).unwrap()
                - parse_timeline("2027-03-15", false).unwrap(),
            1.0
        );
        assert!(parse_timeline("2027-13", false).is_err());
        assert!(parse_timeline("27-03", false).is_err());
        assert!(parse_timeline("2027-02-30", false).is_err());
    }

    #[test]
    fn frise_years() {
        assert_eq!(parse_frise("-1", false).unwrap(), 0.0);
        assert_eq!(parse_frise("1", false).unwrap(), 1.0);
        assert!(parse_frise("0", false).is_err());
        let c = parse_frise("-44-03-15", false).unwrap();
        assert!((c - (-43.0 + 2.0 / 12.0 + 14.0 / 365.0)).abs() < 1e-12);
        assert_eq!(parse_frise("600", true).unwrap(), 601.0);
        assert_eq!(to_historical(0), -1);
        assert_eq!(to_astronomical(-1), 0);
    }
}
