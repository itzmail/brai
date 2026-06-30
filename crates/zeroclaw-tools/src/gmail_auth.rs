use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

struct TokenCache {
    access_token: String,
    expires_at: u64,
}

pub struct GmailTokenStore {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    http: Arc<reqwest::Client>,
    cache: Mutex<TokenCache>,
}

impl GmailTokenStore {
    pub fn new(client_id: String, client_secret: String, refresh_token: String) -> Result<Self> {
        if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
            anyhow::bail!(
                "Gmail credentials missing in config — set channels.gmail.client_id, \
                 channels.gmail.client_secret, channels.gmail.refresh_token in brai.toml"
            );
        }
        let http = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .context("failed to build Gmail HTTP client")?,
        );
        Ok(Self {
            client_id,
            client_secret,
            refresh_token,
            http,
            cache: Mutex::new(TokenCache { access_token: String::new(), expires_at: 0 }),
        })
    }

    pub fn http_client(&self) -> Arc<reqwest::Client> {
        Arc::clone(&self.http)
    }

    pub async fn resolve_access_token(&self) -> Result<String> {
        let mut cache = self.cache.lock().await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_secs();

        if cache.access_token.is_empty() || now + 60 >= cache.expires_at {
            let (access_token, expires_in) =
                exchange_refresh_token(&self.http, &self.client_id, &self.client_secret, &self.refresh_token)
                    .await?;
            cache.access_token = access_token;
            // guard against server returning expires_in=0
            cache.expires_at = now + expires_in.max(60);
        }

        Ok(cache.access_token.clone())
    }
}

async fn exchange_refresh_token(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<(String, u64)> {
    #[derive(serde::Deserialize)]
    struct Resp {
        access_token: String,
        expires_in: Option<u64>,
    }

    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .context("token refresh request failed")?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("token refresh failed: {text}");
    }

    let r: Resp = resp.json().await.context("invalid token response")?;
    Ok((r.access_token, r.expires_in.unwrap_or(3600)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_credentials() {
        assert!(GmailTokenStore::new(String::new(), "s".into(), "r".into()).is_err());
        assert!(GmailTokenStore::new("c".into(), String::new(), "r".into()).is_err());
        assert!(GmailTokenStore::new("c".into(), "s".into(), String::new()).is_err());
    }

    #[test]
    fn accepts_valid_credentials() {
        assert!(GmailTokenStore::new("client_id".into(), "secret".into(), "refresh".into()).is_ok());
    }
}
