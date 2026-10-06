//! Hides secret-looking values in command lines and caps field length (rule S10).
//!
//! Over-redaction is acceptable; leaking a secret into the UI or a log is not.
//! Known gap (deferred, phase 1a round 1 F4): a secret flag followed by another flag and
//! then the value (`--password --verbose VALUE`) is not masked — that shape is ambiguous.

use std::sync::OnceLock;

use regex::{Captures, Regex};

pub const MAX_FIELD_BYTES: usize = 2048;
const MASK: &str = "***";
const ELLIPSIS: &str = "…";

/// Words that make a `name=value` / `--name value` pair secret.
const KEYWORDS: &str = "key|token|secret|pass|pwd|auth|credential|cookie|session";

/// Bare token shapes (provider prefixes) that are secret wherever they start a value.
const TOKEN_SHAPES: &str = concat!(
    r"sk-[A-Za-z0-9_-]{8,}|sk_(?:live|test)_[A-Za-z0-9]{8,}|rk_(?:live|test)_[A-Za-z0-9]{8,}",
    r"|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|glpat-[A-Za-z0-9_-]{16,}",
    r"|npm_[A-Za-z0-9]{30,}|xox[abprs]-[A-Za-z0-9-]{10,}|xapp-[A-Za-z0-9-]{10,}",
    r"|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}",
    r"|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+",
);

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("valid regex"))
}

/// `Authorization: <anything>` — the whole credential, whatever the scheme word.
fn authorization() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        r#"(?i)(authorization["']?\s*[:=]\s*)("[^"]*"?|'[^']*'?|[^'"]+)"#.into()
    })
}

/// `scheme://user:password@host` — the password always, the user when token-shaped.
fn url_userinfo() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        // Empty user allowed (`redis://:pw@host`); the password runs to the last `@`, so
        // passwords containing `/` or `@` are masked whole.
        r#"(?i)\b([a-z][a-z0-9+.-]*://)([^\s/@:'"]*)(:[^\s'"]*)?@"#.into()
    })
}

/// `NAME=value`, `name: value`, `"name": "value"` where NAME contains a keyword.
fn named_secret() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        format!(
            r#"(?i)([A-Za-z0-9_.-]*(?:{KEYWORDS})[A-Za-z0-9_.-]*["']?\s*[=:]\s*)("[^"]*"?|'[^']*'?|\S+)"#
        )
    })
}

/// Inside a command string: `--secret-flag value`, `-p value` / `--port value`.
fn spaced_flag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        format!(
            r#"(?i)(^|\s)(--?[A-Za-z0-9_.-]*(?:{KEYWORDS})[A-Za-z0-9_.-]*|-p|--port)(\s+)([^\s-]\S*)"#
        )
    })
}

/// `-pVALUE` (mysql style), unless the value is a port number.
fn attached_p() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || r"(^|\s)-p([^\s\d-]\S*)".into())
}

/// `-u user:password`, `--user=user:password` — only the part after the colon.
fn user_colon() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        r"(?i)(^|\s)(-u|--user|--username)(\s+|=)([^\s:]+):(\S+)".into()
    })
}

/// `bearer VALUE` / `basic BASE64` written as one argument.
fn scheme_value() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        r#"(?i)\b(bearer\s+)([^\s'"]+)|\b(basic\s+)([A-Za-z0-9+/=]{8,})"#.into()
    })
}

fn slack_webhook() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        r"(?i)(hooks\.slack\.com/services/)[A-Za-z0-9/_-]+".into()
    })
}

/// A token shape at the start of a value (after whitespace, `=`, `:`, `/`, a quote, …).
fn bare_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || format!(r#"(^|[\s=:"'(,;@/])({TOKEN_SHAPES})\b"#))
}

fn whole_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || format!(r"^(?:{TOKEN_SHAPES})$"))
}

/// An argument whose *next* argument is the secret (`--api-key VALUE`, `Bearer VALUE`).
fn secret_flag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    re(&RE, || {
        format!(
            r"(?i)^(-{{1,2}}[A-Za-z0-9_.-]*(?:{KEYWORDS})[A-Za-z0-9_.-]*|bearer|basic|token|digest|authorization:?)$"
        )
    })
}

enum Next {
    Keep,
    Mask,
    /// `-p` / `--port`: keep a port number, mask anything else.
    MaskUnlessPort,
    /// `-u` / `--user`: mask what follows a colon.
    MaskAfterColon,
}

fn is_port(arg: &str) -> bool {
    !arg.is_empty() && arg.len() <= 5 && arg.bytes().all(|b| b.is_ascii_digit())
}

/// Returns a copy of `argv` with secret values replaced by `***` and every element capped.
pub fn redact_argv(argv: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    let mut next = Next::Keep;
    for arg in argv {
        // Invisible characters must not hide a flag name from the checks below.
        let arg: &String = &arg.chars().filter(|c| !is_invisible(*c)).collect();
        let shown = match next {
            Next::Mask => MASK.to_string(),
            Next::MaskUnlessPort if !is_port(arg) => MASK.to_string(),
            Next::MaskAfterColon => match arg.split_once(':') {
                Some((user, _)) => format!("{}:{MASK}", redact_text(user)),
                None => redact_text(arg),
            },
            _ => redact_text(arg),
        };
        next = match (&next, arg.as_str()) {
            (Next::Mask, a) if secret_flag().is_match(a) => Next::Mask,
            (Next::Keep, "-p" | "--port") => Next::MaskUnlessPort,
            (Next::Keep, "-u" | "--user" | "--username") => Next::MaskAfterColon,
            (Next::Keep, a) if secret_flag().is_match(a) => Next::Mask,
            _ => Next::Keep,
        };
        out.push(cap_field(&shown, MAX_FIELD_BYTES));
    }
    out
}

/// Masks secrets inside one piece of free text (an argument, a path, a command string).
pub fn redact_text(text: &str) -> String {
    // Invisible format characters could split a keyword (`to\u{200b}ken=`); drop them.
    let text: String = text.chars().filter(|c| !is_invisible(*c)).collect();
    let text = authorization().replace_all(&text, |c: &Captures| {
        format!("{}{}", &c[1], masked_value(&c[2]))
    });
    let text = url_userinfo().replace_all(&text, |c: &Captures| {
        let user = if whole_token().is_match(&c[2]) {
            MASK
        } else {
            &c[2]
        };
        let pass = if c.get(3).is_some() {
            format!(":{MASK}")
        } else {
            String::new()
        };
        format!("{}{user}{pass}@", &c[1])
    });
    let text = named_secret().replace_all(&text, |c: &Captures| {
        format!("{}{}", &c[1], masked_value(&c[2]))
    });
    let text = user_colon().replace_all(&text, |c: &Captures| {
        format!("{}{}{}{}:{MASK}", &c[1], &c[2], &c[3], &c[4])
    });
    let text = spaced_flag().replace_all(&text, |c: &Captures| {
        let port_flag = matches!(c[2].to_ascii_lowercase().as_str(), "-p" | "--port");
        let value = if port_flag && is_port(&c[4]) {
            &c[4]
        } else {
            MASK
        };
        format!("{}{}{}{value}", &c[1], &c[2], &c[3])
    });
    let text = attached_p().replace_all(&text, |c: &Captures| format!("{}-p{MASK}", &c[1]));
    let text = scheme_value().replace_all(&text, |c: &Captures| match c.get(1) {
        Some(word) => format!("{}{MASK}", word.as_str()),
        None => format!("{}{MASK}", &c[3]),
    });
    let text = slack_webhook().replace_all(&text, |c: &Captures| format!("{}{MASK}", &c[1]));
    mask_bare_tokens(&text)
}

/// Bare tokens, except a path component (`/…/sk-learn-models/…`) followed by another `/`.
fn mask_bare_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for c in bare_token().captures_iter(text) {
        let (prefix, token) = (c.get(1).expect("group"), c.get(2).expect("group"));
        if prefix.as_str() == "/" && text[token.end()..].starts_with('/') {
            continue;
        }
        out.push_str(&text[last..token.start()]);
        out.push_str(MASK);
        last = token.end();
    }
    out.push_str(&text[last..]);
    out
}

/// `***`, keeping the quotes of a quoted value so the surrounding text stays readable.
fn masked_value(value: &str) -> String {
    match value.chars().next() {
        Some(q @ ('"' | '\'')) if value.len() > 1 && value.ends_with(q) => format!("{q}{MASK}{q}"),
        Some(q @ ('"' | '\'')) => format!("{q}{MASK}"),
        _ => MASK.to_string(),
    }
}

/// Zero-width and similar invisible formatting characters.
pub fn is_invisible(c: char) -> bool {
    matches!(c, '\u{00AD}' | '\u{200B}'..='\u{200F}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}')
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
