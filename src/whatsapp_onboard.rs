//! WhatsApp Web onboarding: live pair-code step run right after `brai
//! onboard` writes a new/changed WhatsApp config, so the operator sees the
//! pair-code and a connection confirmation without needing to know about
//! `journalctl -u brai -f`.
//!
//! Lives in the top-level binary crate (not `zeroclaw-runtime::onboard`)
//! because `zeroclaw-channels` (which owns `WhatsAppWebChannel`) depends on
//! `zeroclaw-runtime` — importing `WhatsAppWebChannel` from
//! `zeroclaw-runtime::onboard` would be a circular crate dependency. Only
//! this top-level binary depends on both crates.

/// Strip everything but digits from `raw` and re-prefix with `+`. Returns
/// `None` if no digits remain (e.g. empty input, or input that was only
/// whitespace/punctuation) — callers should warn and skip pairing rather
/// than persist an unusable identity.
pub fn normalize_master_identity(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        None
    } else {
        Some(format!("+{digits}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_leading_plus_and_keeps_digits() {
        assert_eq!(normalize_master_identity("+15550001111"), Some("+15550001111".to_string()));
    }

    #[test]
    fn adds_plus_when_missing() {
        assert_eq!(normalize_master_identity("15550001111"), Some("+15550001111".to_string()));
    }

    #[test]
    fn strips_spaces_and_dashes() {
        assert_eq!(normalize_master_identity("+1 555-000-1111"), Some("+15550001111".to_string()));
    }

    #[test]
    fn empty_input_returns_none() {
        assert_eq!(normalize_master_identity(""), None);
    }

    #[test]
    fn non_digit_input_returns_none() {
        assert_eq!(normalize_master_identity("abc"), None);
    }

    #[test]
    fn already_normalized_is_idempotent() {
        let once = normalize_master_identity("15550001111").unwrap();
        let twice = normalize_master_identity(&once).unwrap();
        assert_eq!(once, twice);
    }
}
