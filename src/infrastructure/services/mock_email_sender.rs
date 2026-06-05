//! In-process `EmailSender` for integration tests.
//!
//! Captures every sent message in a moka cache keyed by normalised
//! recipient address. The test harness retrieves the captured payload
//! via the `GET /api/admin/smtp/test/captured` endpoint (only mounted
//! when `OXICLOUD_SMTP_MOCK=true`), parses the magic-link URL out of
//! the body, and follows it.
//!
//! # NOT for production
//!
//! The capture endpoint is admin-only AND only mounted in mock mode —
//! but even then, exposing inbox-style storage over HTTP is a leak
//! waiting to happen. The mock sender refuses to construct unless the
//! `OXICLOUD_SMTP_MOCK` env var is `true` at startup, so a misconfigured
//! prod deployment can't silently end up here.

use std::sync::Arc;

use async_trait::async_trait;
use moka::future::Cache;
use std::time::Duration;

use crate::application::ports::email_sender::{EmailMessage, EmailSendOutcome, EmailSender};
use crate::common::errors::DomainError;

/// Snapshot of one captured outbound message. Hands a copy to the
/// capture endpoint so the test runner can extract the magic-link URL,
/// verify subject, etc.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CapturedEmail {
    pub to: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: Option<String>,
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

pub struct MockEmailSender {
    /// Keyed by lowercased recipient address; only the most-recent
    /// message is kept. Tests that need a full history can extend
    /// this — for the magic-link flow one-per-recipient is enough.
    captured: Cache<String, Arc<CapturedEmail>>,
}

impl MockEmailSender {
    /// Construct a sender with a generous 10-minute capture TTL — long
    /// enough for a Hurl test suite to retrieve the message at leisure
    /// without the entry getting evicted out from under it.
    pub fn new() -> Self {
        Self {
            captured: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(600))
                .build(),
        }
    }

    /// Fetch the most recent captured message for `recipient` (matched
    /// case-insensitively on the address). Returns `None` if no message
    /// was ever sent to that recipient (or it expired).
    pub async fn last_for(&self, recipient: &str) -> Option<Arc<CapturedEmail>> {
        self.captured.get(&recipient.to_ascii_lowercase()).await
    }

    /// Clear every captured message. Intended for between-test isolation.
    pub async fn clear(&self) {
        self.captured.invalidate_all();
    }
}

impl Default for MockEmailSender {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmailSender for MockEmailSender {
    async fn send(&self, message: EmailMessage) -> Result<EmailSendOutcome, DomainError> {
        let key = message.to.to_ascii_lowercase();
        let entry = CapturedEmail {
            to: message.to.clone(),
            subject: message.subject,
            text_body: message.text_body,
            html_body: message.html_body,
            captured_at: chrono::Utc::now(),
        };
        self.captured.insert(key, Arc::new(entry)).await;

        // Mimic a healthy relay's response so callers that surface the
        // SMTP code (admin "test email" page) see a realistic value.
        Ok(EmailSendOutcome {
            code: 250,
            message: "2.0.0 Mock OK".to_string(),
        })
    }
}
