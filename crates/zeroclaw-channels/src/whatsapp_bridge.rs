//! Channel bridging brai to the external `wa-bridge` service (Baileys-based
//! WhatsApp client running as a separate process). Brai talks to it over
//! HTTP: outbound sends go to `POST {base_url}/send`; inbound messages
//! arrive via the gateway's `POST /webhook/whatsapp` endpoint, which calls
//! `push_inbound()` on this channel's shared instance.
//!
//! `listen()` does not open any socket itself (unlike `WebhookChannel`) —
//! it only captures the orchestrator's mpsc sender into `tx_slot` so
//! `push_inbound()` (called from the gateway, not the orchestrator) can
//! deliver messages through the same channel the orchestrator is reading.

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use brai_api::channel::{Channel, ChannelMessage, SendMessage};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub struct WhatsAppBridgeChannel {
    base_url: String,
    shared_secret: String,
    http: reqwest::Client,
    tx_slot: Arc<Mutex<Option<mpsc::Sender<ChannelMessage>>>>,
}

impl WhatsAppBridgeChannel {
    pub fn new(base_url: String, shared_secret: String) -> Self {
        Self {
            base_url,
            shared_secret,
            http: reqwest::Client::new(),
            tx_slot: Arc::new(Mutex::new(None)),
        }
    }

    /// Called by the gateway's `POST /webhook/whatsapp` handler when
    /// wa-bridge forwards an inbound WhatsApp message. Returns an error if
    /// `listen()` hasn't run yet (orchestrator not wired up) or the
    /// receiver has been dropped.
    pub async fn push_inbound(&self, msg: ChannelMessage) -> Result<()> {
        let guard = self.tx_slot.lock().await;
        let tx = guard
            .as_ref()
            .ok_or_else(|| anyhow!("whatsapp_bridge channel is not listening yet"))?;
        tx.send(msg)
            .await
            .map_err(|_| anyhow!("whatsapp_bridge channel receiver was dropped"))
    }
}

#[async_trait]
impl Channel for WhatsAppBridgeChannel {
    fn name(&self) -> &str {
        "whatsapp_bridge"
    }

    async fn send(&self, message: &SendMessage) -> Result<()> {
        let url = format!("{}/send", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.shared_secret)
            .json(&serde_json::json!({
                "to": message.recipient,
                "text": message.content,
            }))
            .send()
            .await
            .context("wa-bridge /send request failed")?;

        let status = resp.status();
        if status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!("wa-bridge rejected send: recipient not in whitelist");
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("wa-bridge /send failed ({status}): {body}");
        }
        Ok(())
    }

    async fn listen(&self, tx: mpsc::Sender<ChannelMessage>) -> Result<()> {
        *self.tx_slot.lock().await = Some(tx);
        std::future::pending::<()>().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn push_inbound_before_listen_returns_error() {
        let ch = WhatsAppBridgeChannel::new("http://127.0.0.1:1".to_string(), "secret".to_string());
        let msg = ChannelMessage {
            id: "1".to_string(),
            sender: "+15550001111".to_string(),
            reply_target: "+15550001111".to_string(),
            content: "hi".to_string(),
            channel: "whatsapp_bridge".to_string(),
            timestamp: 0,
            thread_ts: None,
            interruption_scope_id: None,
            attachments: vec![],
        };
        let result = ch.push_inbound(msg).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn listen_then_push_inbound_delivers_message() {
        let ch = Arc::new(WhatsAppBridgeChannel::new(
            "http://127.0.0.1:1".to_string(),
            "secret".to_string(),
        ));
        let (tx, mut rx) = mpsc::channel(1);
        let listen_ch = ch.clone();
        tokio::spawn(async move {
            let _ = listen_ch.listen(tx).await;
        });
        // give listen() a moment to store the sender
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let msg = ChannelMessage {
            id: "1".to_string(),
            sender: "+15550001111".to_string(),
            reply_target: "+15550001111".to_string(),
            content: "hi".to_string(),
            channel: "whatsapp_bridge".to_string(),
            timestamp: 0,
            thread_ts: None,
            interruption_scope_id: None,
            attachments: vec![],
        };
        ch.push_inbound(msg.clone()).await.expect("push should succeed");
        let received = rx.recv().await.expect("should receive message");
        assert_eq!(received.content, "hi");
    }
}
