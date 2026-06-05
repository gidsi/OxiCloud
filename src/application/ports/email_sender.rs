//! Outbound email port.
//!
//! Single-recipient transactional mail sending — the entry point for the
//! magic-link invitation flow (PR 9), the login-via-email flow (PR 10), and
//! any future notification mail. Kept deliberately small: one method, one
//! recipient, one body pair (text + optional HTML).
//!
//! The infrastructure-layer implementation lives at
//! `src/infrastructure/services/smtp_email_sender.rs` and is constructed
//! lazily in [`AppServiceFactory`]: when `OXICLOUD_SMTP_HOST` is empty the
//! DI container holds `None`, and endpoints that require email return a
//! clear 503 ("SMTP not configured") rather than silently dropping mail.
//!
//! Future evolution: an in-memory `MemoryEmailSender` for tests (no SMTP
//! round-trip), and a `LoggingEmailSender` decorator that records every
//! send to the audit log. Both are deferred until a concrete consumer
//! needs them.

use async_trait::async_trait;

use crate::common::errors::DomainError;

/// A single outbound message. The `to` address is expected to be a normalised
/// RFC 5321 mailbox (lowercase local-part + punycoded domain); upstream
/// callers handle the normalisation before constructing this struct.
#[derive(Debug, Clone)]
pub struct EmailMessage {
    /// RFC 5321 recipient address. Single recipient per send today — the
    /// invite flow targets one external user at a time. Multi-recipient
    /// (CC/BCC) is intentionally out of scope.
    pub to: String,
    /// Plain-text subject line. UTF-8 — lettre handles RFC 2047 encoding.
    pub subject: String,
    /// Plain-text body. Always required; mail clients without HTML
    /// rendering fall back to this.
    pub text_body: String,
    /// Optional HTML body. When present, the message is sent as
    /// `multipart/alternative` with both representations.
    pub html_body: Option<String>,
}

/// What the SMTP server said when it accepted the message. Surfaced
/// through the trait so the admin "test email" endpoint can show the
/// response to operators; the invitation flow generally ignores it but
/// logs it via `tracing`.
#[derive(Debug, Clone)]
pub struct EmailSendOutcome {
    /// SMTP status code from the final response (e.g. `250` for "OK").
    /// Encoded as a `u16` because that's the natural range; lettre
    /// returns it as a structured enum and we collapse it here.
    pub code: u16,
    /// First line of the server's reply (e.g. `"2.0.0 OK"`, or the
    /// upstream provider's queue-id banner). Best-effort; if the
    /// response was empty (unusual) this is the empty string.
    pub message: String,
}

/// Port for sending transactional email.
///
/// Implementations must:
/// - Be idempotent at the network level (lettre handles connection reuse).
/// - Run the actual SMTP exchange on the existing tokio runtime (no
///   blocking threads).
/// - Return `DomainError` with `ErrorKind::ExternalService` (or the most
///   precise variant available) on permanent failures so handlers can
///   distinguish "couldn't reach SMTP" from validation errors.
///
/// `#[async_trait]` is used so the trait is dyn-compatible — the DI
/// container holds `Arc<dyn EmailSender>` (matches the existing
/// `dyn` patterns at the service boundary).
#[async_trait]
pub trait EmailSender: Send + Sync + 'static {
    /// Send one message. Returns `Ok(outcome)` only after the SMTP server
    /// has accepted the message (i.e. after the final `.` or LMTP DATA
    /// close). The outcome carries the SMTP response code + first line
    /// so diagnostic surfaces (admin "test email" page) can show it.
    /// Caller may run this fire-and-forget via `tokio::spawn` if response
    /// timing matters (e.g. magic-link invite path defending against
    /// enumeration via latency); the outcome is then logged-only.
    async fn send(&self, message: EmailMessage) -> Result<EmailSendOutcome, DomainError>;
}
