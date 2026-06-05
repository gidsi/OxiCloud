//! Lettre-backed implementation of [`EmailSender`].
//!
//! Wraps an [`AsyncSmtpTransport`] configured from [`SmtpConfig`] at
//! application startup. The transport itself is internally connection-
//! pooled, so a single instance is shared across the whole app via the
//! DI container.
//!
//! On startup the `From:` mailbox is parsed once and cached. Bad config
//! (unparseable `from`, missing `host`) is reported during construction
//! so the server fails fast rather than at first send.
//!
//! # No retry / no spool — by design
//!
//! `send()` makes a single attempt against the configured relay. If the
//! relay is unreachable, slow, or returns a transient error, the call
//! returns `Err` and the message is gone. There is no in-process queue,
//! no exponential backoff, no dead-letter handling.
//!
//! Operators who need durability across upstream relay outages should
//! point `OXICLOUD_SMTP_HOST` at a local MTA (Postfix, OpenSMTPD,
//! msmtp-mta, …) configured as a smarthost — the local MTA owns the
//! retry queue. See `docs/config/env.md` → "Reliability and retries"
//! for the recipe.

use async_trait::async_trait;
use lettre::message::{Mailbox, MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncTransport, Message, Tokio1Executor};

use crate::application::ports::email_sender::{EmailMessage, EmailSendOutcome, EmailSender};
use crate::common::config::{SmtpConfig, SmtpTlsMode};
use crate::common::errors::DomainError;

pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    /// Parsed once at construction so every send reuses the same
    /// `Mailbox` value (and any RFC 5322 name-address parsing errors
    /// surface during startup instead of at first send).
    from: Mailbox,
}

impl SmtpEmailSender {
    /// Build a sender from an SMTP config block. Returns an `Err` when
    /// `from` is unparseable or the transport's TLS parameters can't be
    /// constructed — both surface at startup so misconfiguration never
    /// silently drops mail.
    pub fn new(cfg: &SmtpConfig) -> Result<Self, DomainError> {
        if cfg.host.is_empty() {
            return Err(DomainError::internal_error(
                "SmtpEmailSender",
                "OXICLOUD_SMTP_HOST is empty — refusing to construct a no-op sender",
            ));
        }

        let from: Mailbox = cfg.from.parse().map_err(|e| {
            DomainError::internal_error(
                "SmtpEmailSender",
                format!("invalid OXICLOUD_SMTP_FROM mailbox '{}': {}", cfg.from, e),
            )
        })?;

        let builder = match cfg.tls {
            SmtpTlsMode::Starttls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host).map_err(|e| {
                    DomainError::internal_error(
                        "SmtpEmailSender",
                        format!("starttls relay for {}: {}", cfg.host, e),
                    )
                })?
            }
            SmtpTlsMode::Tls => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host).map_err(|e| {
                    DomainError::internal_error(
                        "SmtpEmailSender",
                        format!("tls relay for {}: {}", cfg.host, e),
                    )
                })?
            }
            SmtpTlsMode::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host),
        };

        let mut builder = builder.port(cfg.port);
        if !cfg.user.is_empty() {
            builder = builder.credentials(Credentials::new(cfg.user.clone(), cfg.pass.clone()));
        }
        let transport = builder.build();

        Ok(Self { transport, from })
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(&self, message: EmailMessage) -> Result<EmailSendOutcome, DomainError> {
        let to: Mailbox = message.to.parse().map_err(|e| {
            DomainError::new(
                crate::common::errors::ErrorKind::InvalidInput,
                "SmtpEmailSender",
                format!("invalid recipient '{}': {}", message.to, e),
            )
        })?;

        let builder = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(message.subject.clone());

        // multipart/alternative when an HTML body is supplied — old text
        // clients see the text part, modern clients render the HTML.
        let built = match message.html_body {
            Some(html) => builder.multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(message.text_body),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            ),
            None => builder.singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(message.text_body),
            ),
        }
        .map_err(|e| {
            DomainError::internal_error("SmtpEmailSender", format!("build message: {}", e))
        })?;

        let response =
            self.transport.send(built).await.map_err(|e| {
                DomainError::internal_error("SmtpEmailSender", format!("send: {}", e))
            })?;

        // Lettre's `Response::code()` returns a structured `Code`; its
        // `Display` impl is the three-digit form ("250", "451", …).
        let code: u16 = response.code().to_string().parse().unwrap_or(0);
        // `message()` is `Iterator<Item = &String>`; take the first
        // line (the rest are typically multi-line EHLO continuations,
        // not interesting for a confirmation).
        let message = response
            .message()
            .next()
            .map(str::to_string)
            .unwrap_or_default();

        Ok(EmailSendOutcome { code, message })
    }
}
