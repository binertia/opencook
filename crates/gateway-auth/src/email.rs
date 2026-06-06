//! Email dispatch service for password reset and notifications.

use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

/// Email service configuration.
#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

/// Email dispatch service.
#[derive(Debug, Clone)]
pub struct EmailService {
    config: EmailConfig,
}

impl EmailService {
    /// Create a new email service from configuration.
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }

    /// Send a password reset email.
    pub async fn send_password_reset(&self, to: &str, reset_url: &str) -> Result<(), EmailError> {
        let body = format!(
            "You requested a password reset for your AI Gateway account.\n\n\
             Click the link below to reset your password (expires in 1 hour):\n\n\
             {}\n\n\
             If you did not request this, you can safely ignore this email.\n",
            reset_url
        );

        self.send_email(to, "Password Reset Request", &body).await
    }

    /// Send a generic plain-text email.
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError> {
        let to_mailbox: Mailbox = to
            .parse()
            .map_err(|_| EmailError::InvalidAddress(to.to_string()))?;

        let from_mailbox: Mailbox = self
            .config
            .from
            .parse()
            .map_err(|_| EmailError::InvalidAddress(self.config.from.clone()))?;

        let message = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .body(body.to_string())
            .map_err(|e| EmailError::Build(e.to_string()))?;

        let creds = match (&self.config.user, &self.config.password) {
            (Some(user), Some(pass)) => Some(Credentials::new(user.clone(), pass.clone())),
            _ => None,
        };

        let mut builder =
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                .port(self.config.port);

        if let Some(creds) = creds {
            builder = builder.credentials(creds);
        }

        let transport = builder.build();

        transport.send(message).await.map_err(|e| {
            tracing::error!(error = %e, "Failed to send email");
            EmailError::Transport(e.to_string())
        })?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),
    #[error("Failed to build email: {0}")]
    Build(String),
    #[error("SMTP transport error: {0}")]
    Transport(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config() {
        let config = EmailConfig {
            host: "smtp.example.com".to_string(),
            port: 587,
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
            from: "noreply@example.com".to_string(),
        };
        let svc = EmailService::new(config);
        assert_eq!(svc.config.host, "smtp.example.com");
    }
}
