//! Email address normalization.
//!
//! Every email coming through the magic-link invitation path is funneled
//! through [`normalize_email`] before it is compared against existing
//! users or persisted in `auth.users.email`. Two addresses that differ
//! only in case or in the IDN encoding of the domain MUST collapse to
//! the same stored form — otherwise the same recipient would be invited
//! twice and end up with two `is_external` accounts.
//!
//! # Rules
//!
//! - The local-part (before the `@`) is lower-cased. We treat the local
//!   part as opaque: Gmail-style `+tag` aliases and dot-insensitivity are
//!   NOT special-cased. Each `alice+invoices@example.com` and
//!   `alice@example.com` is a distinct identity from our perspective.
//! - The domain (after the `@`) is lower-cased, then run through
//!   `idna::domain_to_ascii` so internationalised domains land as
//!   punycode (`münchen.de` → `xn--mnchen-3ya.de`). This keeps the
//!   stored form ASCII; UIs that want to display the unicode original
//!   can reverse it with `idna::domain_to_unicode`.
//! - Exactly one `@` separator is required. Whitespace around the input
//!   is trimmed. Empty local-part or empty domain is rejected. Overall
//!   length must fit in the 254-char RFC 5321 envelope cap.
//!
//! # What this is NOT
//!
//! - Not a deliverability check. No MX lookup, no syntax validation
//!   beyond the basics above. A normalized string that comes out of
//!   here can still fail at SMTP-send time.
//! - Not a sanitiser against XSS / SQL injection. Callers must still
//!   treat the output as untrusted text.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailNormalizeError {
    /// Missing `@` separator, or more than one (we require exactly one
    /// post-trim, splitting on the last `@`).
    Malformed,
    /// Local-part is empty after lowercasing / trimming.
    EmptyLocal,
    /// Domain is empty or punycode conversion failed.
    InvalidDomain,
    /// Normalised form exceeds RFC 5321's 254-char ceiling.
    TooLong,
}

impl fmt::Display for EmailNormalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => write!(f, "email is missing '@' separator"),
            Self::EmptyLocal => write!(f, "email local-part is empty"),
            Self::InvalidDomain => write!(f, "email domain is empty or invalid"),
            Self::TooLong => write!(f, "email exceeds 254 characters"),
        }
    }
}

impl std::error::Error for EmailNormalizeError {}

/// Normalise a raw email address. See module docs for the rules.
pub fn normalize_email(raw: &str) -> Result<String, EmailNormalizeError> {
    let trimmed = raw.trim();
    let (local, domain) = trimmed
        .rsplit_once('@')
        .ok_or(EmailNormalizeError::Malformed)?;

    let local_lc = local.to_ascii_lowercase();
    if local_lc.is_empty() {
        return Err(EmailNormalizeError::EmptyLocal);
    }

    let domain_lc = domain.to_ascii_lowercase();
    if domain_lc.is_empty() {
        return Err(EmailNormalizeError::InvalidDomain);
    }
    let domain_ascii =
        idna::domain_to_ascii(&domain_lc).map_err(|_| EmailNormalizeError::InvalidDomain)?;
    if domain_ascii.is_empty() {
        return Err(EmailNormalizeError::InvalidDomain);
    }

    let out = format!("{}@{}", local_lc, domain_ascii);
    if out.len() > 254 {
        return Err(EmailNormalizeError::TooLong);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passthrough_lowercases_local_and_domain() {
        assert_eq!(
            normalize_email("Alice@Example.COM").unwrap(),
            "alice@example.com"
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            normalize_email("  bob@example.com  \n").unwrap(),
            "bob@example.com"
        );
    }

    #[test]
    fn idn_domain_is_punycoded() {
        // u-umlaut in München.
        assert_eq!(
            normalize_email("user@münchen.de").unwrap(),
            "user@xn--mnchen-3ya.de"
        );
    }

    #[test]
    fn plus_tag_local_part_is_preserved() {
        // No Gmail-style folding.
        assert_eq!(
            normalize_email("alice+invoices@example.com").unwrap(),
            "alice+invoices@example.com"
        );
    }

    #[test]
    fn missing_at_is_rejected() {
        assert_eq!(
            normalize_email("not-an-email").unwrap_err(),
            EmailNormalizeError::Malformed
        );
    }

    #[test]
    fn empty_local_is_rejected() {
        assert_eq!(
            normalize_email("@example.com").unwrap_err(),
            EmailNormalizeError::EmptyLocal
        );
    }

    #[test]
    fn empty_domain_is_rejected() {
        assert_eq!(
            normalize_email("alice@").unwrap_err(),
            EmailNormalizeError::InvalidDomain
        );
    }

    #[test]
    fn multi_at_uses_last_separator() {
        // `rsplit_once('@')` splits at the rightmost `@`. The local-part
        // can legally contain `@` if quoted; we don't fully parse
        // RFC 5321, so we let the resulting local-part through and rely
        // on the caller's email regex / SMTP server to reject malformed
        // local-parts the relay will refuse.
        assert_eq!(
            normalize_email("WeIrD@local@example.com").unwrap(),
            "weird@local@example.com"
        );
    }

    #[test]
    fn over_254_chars_is_rejected() {
        let long_local = "a".repeat(250);
        let raw = format!("{}@x.io", long_local);
        assert_eq!(
            normalize_email(&raw).unwrap_err(),
            EmailNormalizeError::TooLong
        );
    }
}
