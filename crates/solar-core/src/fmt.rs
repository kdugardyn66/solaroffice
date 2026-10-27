//! Human-readable sizes, dates and the home greeting.

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};

/// `1.2 MB`, `640 KB`, `12 B`.
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64 / 1024.0;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 { format!("{v:.1} {}", UNITS[i]) } else { format!("{v:.0} {}", UNITS[i]) }
}

/// A timestamp relative to `now`: `Just now`, `Today 14:05`, `Yesterday 09:12`, `12 Oct`,
/// `12 Oct 2025`.
pub fn when<Tz: TimeZone>(ms: i64, now: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let tz = now.timezone();
    let Some(t) = tz.timestamp_millis_opt(ms).single() else { return String::new() };
    let secs = (now.timestamp_millis() - ms) / 1000;
    if (0..60).contains(&secs) {
        return "Just now".into();
    }
    let today = now.date_naive();
    let day = t.date_naive();
    if day == today {
        format!("Today {:02}:{:02}", t.hour(), t.minute())
    } else if today.pred_opt() == Some(day) {
        format!("Yesterday {:02}:{:02}", t.hour(), t.minute())
    } else if day.year() == today.year() {
        t.format("%-d %b").to_string()
    } else {
        t.format("%-d %b %Y").to_string()
    }
}

/// [`when`] in the local time zone.
pub fn when_local(ms: i64) -> String {
    when(ms, &Local::now())
}

/// The greeting for `hour` (0–23): `Good morning, Kris.`
pub fn greeting(hour: u32, name: Option<&str>) -> String {
    let part = match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    };
    match name {
        Some(n) => format!("{part}, {n}."),
        None => format!("{part}."),
    }
}

/// The question under the greeting; one is picked per launch.
pub const ASK_LINES: [&str; 6] = [
    "What would you like to create today?",
    "Ready to get started?",
    "What are you working on?",
    "Where shall we begin today?",
    "Feeling creative?",
    "What's next on your mind?",
];

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn sizes() {
        assert_eq!(size(12), "12 B");
        assert_eq!(size(2048), "2.0 KB");
        assert_eq!(size(640 * 1024), "640 KB");
        assert_eq!(size(1_258_291), "1.2 MB");
    }

    #[test]
    fn relative_dates() {
        let now = Utc.with_ymd_and_hms(2026, 10, 23, 15, 0, 0).unwrap();
        let ms = |y, mo, d, h, mi| Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().timestamp_millis();
        assert_eq!(when(now.timestamp_millis() - 5_000, &now), "Just now");
        assert_eq!(when(ms(2026, 10, 23, 9, 5), &now), "Today 09:05");
        assert_eq!(when(ms(2026, 10, 22, 23, 59), &now), "Yesterday 23:59");
        assert_eq!(when(ms(2026, 3, 4, 1, 0), &now), "4 Mar");
        assert_eq!(when(ms(2025, 12, 31, 1, 0), &now), "31 Dec 2025");
    }

    #[test]
    fn greetings() {
        assert_eq!(greeting(8, Some("Kris")), "Good morning, Kris.");
        assert_eq!(greeting(13, None), "Good afternoon.");
        assert_eq!(greeting(23, Some("Kris")), "Good evening, Kris.");
        assert_eq!(greeting(2, None), "Good evening.");
    }
}
