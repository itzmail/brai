use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use brai_api::tool::{Tool, ToolResult};
use brai_config::scattered_types::GmailPushConfig;

/// Gmail read tool — polls Gmail API for recent messages.
///
/// Uses OAuth access token from GmailPushConfig. Auto-refreshes token
/// if refresh_token + client credentials are configured.
pub struct GmailReadTool {
    config: Arc<GmailPushConfig>,
    http: reqwest::Client,
}

impl GmailReadTool {
    pub fn new(config: Arc<GmailPushConfig>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self { config, http }
    }

    /// Resolve a valid access token — tries refresh_token first, falls back to static token.
    async fn resolve_token(&self) -> anyhow::Result<String> {
        let refresh_token = if !self.config.refresh_token.is_empty() {
            self.config.refresh_token.clone()
        } else {
            std::env::var("GMAIL_PUSH_REFRESH_TOKEN").unwrap_or_default()
        };
        let client_id = if !self.config.client_id.is_empty() {
            self.config.client_id.clone()
        } else {
            std::env::var("GMAIL_PUSH_CLIENT_ID").unwrap_or_default()
        };
        let client_secret = if !self.config.client_secret.is_empty() {
            self.config.client_secret.clone()
        } else {
            std::env::var("GMAIL_PUSH_CLIENT_SECRET").unwrap_or_default()
        };

        if !refresh_token.is_empty() && !client_id.is_empty() && !client_secret.is_empty() {
            return self.refresh_access_token(&refresh_token, &client_id, &client_secret).await;
        }

        let token = if !self.config.oauth_token.is_empty() {
            self.config.oauth_token.clone()
        } else {
            std::env::var("GMAIL_PUSH_OAUTH_TOKEN").unwrap_or_default()
        };

        if token.is_empty() {
            anyhow::bail!("Gmail OAuth token not configured");
        }
        Ok(token)
    }

    async fn refresh_access_token(&self, refresh_token: &str, client_id: &str, client_secret: &str) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct TokenResp { access_token: String }

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];
        let resp = self.http
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Token refresh failed: {text}");
        }
        let t: TokenResp = resp.json().await?;
        Ok(t.access_token)
    }

    async fn list_messages(&self, token: &str, max: u32, query: &str) -> anyhow::Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct ListResp { messages: Option<Vec<MsgRef>> }
        #[derive(serde::Deserialize)]
        struct MsgRef { id: String }

        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages?maxResults={max}&q={query}"
        );
        let resp = self.http.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Gmail list failed: {text}");
        }
        let list: ListResp = resp.json().await?;
        Ok(list.messages.unwrap_or_default().into_iter().map(|m| m.id).collect())
    }

    async fn fetch_message(&self, token: &str, id: &str) -> anyhow::Result<ParsedEmail> {
        #[derive(serde::Deserialize)]
        struct Msg {
            snippet: Option<String>,
            payload: Option<Payload>,
            #[serde(rename = "internalDate")]
            internal_date: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct Payload { headers: Option<Vec<Header>> }
        #[derive(serde::Deserialize)]
        struct Header { name: String, value: String }

        let url = format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=Subject&metadataHeaders=Date");
        let resp = self.http.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Gmail fetch failed: {text}");
        }
        let msg: Msg = resp.json().await?;
        let headers = msg.payload.and_then(|p| p.headers).unwrap_or_default();
        let get_header = |name: &str| -> String {
            headers.iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .map(|h| h.value.clone())
                .unwrap_or_default()
        };

        Ok(ParsedEmail {
            from: get_header("From"),
            subject: get_header("Subject"),
            date: get_header("Date"),
            snippet: msg.snippet.unwrap_or_default(),
        })
    }
}

struct ParsedEmail {
    from: String,
    subject: String,
    date: String,
    snippet: String,
}

#[async_trait::async_trait]
impl Tool for GmailReadTool {
    fn name(&self) -> &str {
        "gmail_read"
    }

    fn description(&self) -> &str {
        "Read recent emails from Gmail. Returns sender, subject, date, and snippet. \
         Use query parameter to filter (e.g. 'is:unread', 'from:bank', 'subject:transaksi'). \
         Useful for checking financial notifications, reading important emails."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "max_results": {
                    "type": "integer",
                    "description": "Number of emails to return (default: 10, max: 50)",
                    "default": 10
                },
                "query": {
                    "type": "string",
                    "description": "Gmail search query (e.g. 'is:unread', 'from:bca', 'subject:transaksi newer_than:1d')",
                    "default": "is:unread"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let max = args.get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(50) as u32;

        let query = args.get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("is:unread")
            .to_string();

        let token = match self.resolve_token().await {
            Ok(t) => t,
            Err(e) => return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        };

        let ids = match self.list_messages(&token, max, &query).await {
            Ok(ids) => ids,
            Err(e) => return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        };

        if ids.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "No emails found.".into(),
                error: None,
            });
        }

        let mut results = Vec::with_capacity(ids.len());
        for id in &ids {
            match self.fetch_message(&token, id).await {
                Ok(email) => {
                    results.push(format!(
                        "From: {}\nSubject: {}\nDate: {}\nSnippet: {}\n",
                        email.from, email.subject, email.date, email.snippet
                    ));
                }
                Err(e) => {
                    results.push(format!("[Error fetching {id}: {e}]\n"));
                }
            }
        }

        Ok(ToolResult {
            success: true,
            output: results.join("---\n"),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brai_config::scattered_types::GmailPushConfig;

    #[test]
    fn tool_name() {
        let tool = GmailReadTool::new(Arc::new(GmailPushConfig::default()));
        assert_eq!(tool.name(), "gmail_read");
    }

    #[test]
    fn parameters_schema_valid() {
        let tool = GmailReadTool::new(Arc::new(GmailPushConfig::default()));
        let schema = tool.parameters_schema();
        assert!(schema.get("properties").is_some());
    }
}
