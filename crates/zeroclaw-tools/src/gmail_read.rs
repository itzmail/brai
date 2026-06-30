use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use brai_api::tool::{Tool, ToolResult};
use crate::gmail_auth::GmailTokenStore;

pub struct GmailReadTool {
    tokens: Arc<GmailTokenStore>,
    http: Arc<reqwest::Client>,
}

impl GmailReadTool {
    pub fn new(tokens: Arc<GmailTokenStore>) -> Self {
        let http = tokens.http_client();
        Self { tokens, http }
    }

    async fn list_messages(&self, token: &str, max: u32, query: &str) -> anyhow::Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct ListResp { messages: Option<Vec<MsgRef>> }
        #[derive(serde::Deserialize)]
        struct MsgRef { id: String }

        let max_str = max.to_string();
        let resp = self.http
            .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
            .query(&[("maxResults", max_str.as_str()), ("q", query)])
            .bearer_auth(token)
            .send()
            .await?;
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
        }
        #[derive(serde::Deserialize)]
        struct Payload { headers: Option<Vec<Header>> }
        #[derive(serde::Deserialize)]
        struct Header { name: String, value: String }

        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=Subject&metadataHeaders=Date"
        );
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

#[async_trait]
impl Tool for GmailReadTool {
    fn name(&self) -> &str { "gmail_read" }

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

        let token = match self.tokens.resolve_access_token().await {
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
                Err(e) => results.push(format!("[Error fetching {id}: {e}]\n")),
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

    #[test]
    fn tool_name() {
        let store = Arc::new(GmailTokenStore::new("c".into(), "s".into(), "r".into()).unwrap());
        let tool = GmailReadTool::new(store);
        assert_eq!(tool.name(), "gmail_read");
    }

    #[test]
    fn parameters_schema_valid() {
        let store = Arc::new(GmailTokenStore::new("c".into(), "s".into(), "r".into()).unwrap());
        let tool = GmailReadTool::new(store);
        let schema = tool.parameters_schema();
        assert!(schema.get("properties").is_some());
    }
}
