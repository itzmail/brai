use async_trait::async_trait;
use brai_api::tool::{Tool, ToolResult};
use serde_json::json;

pub struct WhatsAppSendTool {
    base_url: String,
    shared_secret: String,
    http: reqwest::Client,
}

impl WhatsAppSendTool {
    pub fn new(base_url: String, shared_secret: String) -> Self {
        Self {
            base_url,
            shared_secret,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for WhatsAppSendTool {
    fn name(&self) -> &str {
        "whatsapp_send"
    }

    fn description(&self) -> &str {
        "Send a WhatsApp message to any phone number via the wa-bridge service. \
         Provide the recipient's phone number (E.164 format, e.g. +15550001111) \
         and the message text."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient phone number, E.164 format"
                },
                "text": {
                    "type": "string",
                    "description": "Message text to send"
                }
            },
            "required": ["to", "text"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let to = match args.get("to").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("missing 'to'".into()),
                });
            }
        };
        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("missing 'text'".into()),
                });
            }
        };

        let url = format!("{}/send", self.base_url.trim_end_matches('/'));
        let resp = match self
            .http
            .post(&url)
            .bearer_auth(&self.shared_secret)
            .json(&json!({ "to": to, "text": text }))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("wa-bridge request failed: {e}")),
                });
            }
        };

        let status = resp.status();
        if status == reqwest::StatusCode::FORBIDDEN {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Recipient not in wa-bridge whitelist".to_string()),
            });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("wa-bridge send failed ({status}): {body}")),
            });
        }

        Ok(ToolResult {
            success: true,
            output: format!("Message sent to {to}"),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_to_returns_error() {
        let tool = WhatsAppSendTool::new("http://127.0.0.1:1".to_string(), "secret".to_string());
        let result = tool.execute(json!({"text": "hi"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("to"));
    }

    #[tokio::test]
    async fn missing_text_returns_error() {
        let tool = WhatsAppSendTool::new("http://127.0.0.1:1".to_string(), "secret".to_string());
        let result = tool.execute(json!({"to": "+15550001111"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("text"));
    }

    #[tokio::test]
    async fn unreachable_base_url_returns_generic_error() {
        let tool = WhatsAppSendTool::new("http://127.0.0.1:1".to_string(), "secret".to_string());
        let result = tool
            .execute(json!({"to": "+15550001111", "text": "hi"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
