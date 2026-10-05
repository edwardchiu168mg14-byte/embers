//! Turns the facts behind a decision into one short, human-readable sentence.

pub const MAX_REASON_CHARS: usize = 160;
const SEP: &str = " · ";

/// Joins facts with a middle dot, dropping duplicates, capped at [`MAX_REASON_CHARS`].
pub fn sentence(facts: &[String]) -> String {
    let mut out = String::new();
    let mut seen: Vec<&str> = Vec::new();
    for fact in facts {
        if fact.is_empty() || seen.contains(&fact.as_str()) {
            continue;
        }
        seen.push(fact);
        if out.is_empty() {
            // The first fact always appears, shortened if it alone is too long.
            out = truncate_chars(fact, MAX_REASON_CHARS);
            continue;
        }
        let piece = format!("{SEP}{fact}");
        if out.chars().count() + piece.chars().count() > MAX_REASON_CHARS {
            break;
        }
        out.push_str(&piece);
    }
    out
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// `8_040_000` ms → `"2 h 14 m"`; under an hour → `"45 m"`; under a minute → `"30 s"`.
pub fn duration(ms: u64) -> String {
    let secs = ms / 1000;
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, 0) => format!("{secs} s"),
        (0, m) => format!("{m} m"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} m"),
    }
}

/// Binary units labelled like Activity Monitor: `30 GB`, `8.5 GB`, `512 MB`.
pub fn bytes(b: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = b as f64;
    if b >= 10.0 * GIB {
        format!("{:.0} GB", b / GIB)
    } else if b >= GIB {
        format!("{:.1} GB", b / GIB)
    } else {
        format!("{:.0} MB", b / MIB)
    }
}

pub fn cpu(pct: f32) -> String {
    format!("{pct:.0} % CPU")
}
