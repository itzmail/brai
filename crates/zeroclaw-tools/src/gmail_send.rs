use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as BASE64};
use serde_json::json;
use std::sync::Arc;
use brai_api::tool::{Tool, ToolResult};
use crate::gmail_auth::GmailTokenStore;

pub struct GmailSendTool {
    tokens: Arc<GmailTokenStore>,
    http: Arc<reqwest::Client>,
}

impl GmailSendTool {
    pub fn new(tokens: Arc<GmailTokenStore>) -> Self {
        let http = tokens.http_client();
        Self { tokens, http }
    }

    async fn send_email(&self, token: &str, to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
        let raw = format!(
            "To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}"
        );
        let encoded = BASE64.encode(raw.as_bytes());

        let payload = json!({ "raw": encoded });
        let resp = self.http
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Gmail send failed: {text}");
        }
        Ok(())
    }
}

#[async_trait]
impl Tool for GmailSendTool {
    fn name(&self) -> &str { "gmail_send" }

    fn description(&self) -> &str {
        "Send an email via Gmail. Provide recipient address, subject, and plain-text body."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient email address"
                },
                "subject": {
                    "type": "string",
                    "description": "Email subject"
                },
                "body": {
                    "type": "string",
                    "description": "Plain-text email body"
                }
            },
            "required": ["to", "subject", "body"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let to = match args.get("to").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => return Ok(ToolResult { success: false, output: String::new(), error: Some("missing 'to'".into()) }),
        };
        let subject = match args.get("subject").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => return Ok(ToolResult { success: false, output: String::new(), error: Some("missing 'subject'".into()) }),
        };
        let body = match args.get("body").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => return Ok(ToolResult { success: false, output: String::new(), error: Some("missing 'body'".into()) }),
        };

        let token = match self.tokens.resolve_access_token().await {
            Ok(t) => t,
            Err(e) => return Ok(ToolResult { success: false, output: String::new(), error: Some(e.to_string()) }),
        };

        match self.send_email(&token, &to, &subject, &body).await {
            Ok(()) => Ok(ToolResult {
                success: true,
                output: format!("Email sent to {to}"),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        let store = Arc::new(GmailTokenStore::new("c".into(), "s".into(), "r".into()).unwrap());
        assert_eq!(GmailSendTool::new(store).name(), "gmail_send");
    }

    #[test]
    fn schema_requires_fields() {
        let store = Arc::new(GmailTokenStore::new("c".into(), "s".into(), "r".into()).unwrap());
        let schema = GmailSendTool::new(store).parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "to"));
        assert!(required.iter().any(|v| v == "subject"));
        assert!(required.iter().any(|v| v == "body"));
    }
}
