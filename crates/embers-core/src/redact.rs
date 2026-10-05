//! Hides secret-looking values in command lines and caps field length (rule S10).
//!
//! Over-redaction is acceptable; leaking a secret into the UI or a log is not.

use std::sync::OnceLock;

use regex::Regex;

pub const MAX_FIELD_BYTES: usize = 2048;
const MASK: &str = "***";
const ELLIPSIS: &str = "…";

/// `KEY=value`, `key: value`, `Authorization: Bearer value` inside one argument.
fn inline_secret() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)([A-Za-z0-9_.-]*(?:key|token|secret|passw(?:or)?d|auth|credential|cookie|session)[A-Za-z0-9_.-]*["']?\s*[=:]\s*["']?(?:bearer\s+|basic\s+)?)([^\s"'&;]+)"#,
        )
        .expect("valid regex")
    })
}

/// Token shapes that are secret on their own. Only matched at the start of a value
/// (after whitespace, `=`, `:`, a quote, …), so look-alikes inside paths are left alone.
fn bare_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(^|[\s=:"'(,;@])(sk-[A-Za-z0-9_-]{8,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[abprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+)\b"#,
        )
        .expect("valid regex")
    })
}

/// A flag such as `--api-key` or `-p` whose *next* argument is the secret.
fn secret_flag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(-{1,2}[A-Za-z0-9_.-]*(key|token|secret|passw(or)?d|auth|credential|cookie)[A-Za-z0-9_.-]*|-p|bearer|basic|authorization:?)$")
            .expect("valid regex")
    })
}

/// Returns a copy of `argv` with secret values replaced by `***` and every element capped.
pub fn redact_argv(argv: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    let mut mask_next = false;
    for arg in argv {
        if mask_next {
            mask_next = secret_flag().is_match(arg);
            out.push(MASK.to_string());
            continue;
        }
        mask_next = secret_flag().is_match(arg);
        out.push(cap_field(&redact_text(arg), MAX_FIELD_BYTES));
    }
    out
}

/// Masks inline `name=value` / `name: value` secrets and known token shapes in free text.
pub fn redact_text(text: &str) -> String {
    let named = inline_secret().replace_all(text, |c: &regex::Captures| format!("{}{MASK}", &c[1]));
    bare_token()
        .replace_all(&named, |c: &regex::Captures| format!("{}{MASK}", &c[1]))
        .into_owned()
}

/// Truncates `text` to at most `max_bytes` bytes (on a char boundary), ending with `…`
/// when cut. Content is kept as plain data; nothing is stripped or interpreted.
pub fn cap_field(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes.saturating_sub(ELLIPSIS.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", &text[..end])
}
