//! WhatsApp Web onboarding: live pair-code step run right after `brai
//! onboard` writes a new/changed WhatsApp config, so the operator sees the
//! pair-code and a connection confirmation without needing to know about
//! `journalctl -u brai -f`.
//!
//! Lives in the top-level binary crate (not `zeroclaw-runtime::onboard`)
//! because `zeroclaw-channels` (which owns `WhatsAppWebChannel`) depends on
//! `zeroclaw-runtime` — importing `WhatsAppWebChannel` from
//! `zeroclaw-runtime::onboard` would be a circular crate dependency. Only
//! this top-level binary depends on both crates.

/// Strip everything but digits from `raw` and re-prefix with `+`. Returns
/// `None` if no digits remain (e.g. empty input, or input that was only
/// whitespace/punctuation) — callers should warn and skip pairing rather
/// than persist an unusable identity.
pub fn normalize_master_identity(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        None
    } else {
        Some(format!("+{digits}"))
    }
}

/// Connect once to WhatsApp Web using `wa`'s configuration, show the
/// pair-code as it arrives, and wait (up to 60s per phase) for a confirmed
/// connection — printing everything through `ui` so it renders in whichever
/// onboard UI backend the caller is already using.
///
/// Never returns `Err` in a way that should abort `brai onboard` — config
/// is already saved by the time this runs; pairing failure is reported and
/// offered a retry, not treated as a fatal error.
#[cfg(feature = "whatsapp-web")]
pub async fn run_whatsapp_pairing_step(
    wa: &brai_config::schema::WhatsAppConfig,
    ui: &mut dyn brai_config::traits::OnboardUi,
) -> anyhow::Result<()> {
    use brai_api::channel::Channel;
    use brai_channels::whatsapp_web::WhatsAppWebChannel;
    use std::sync::Arc;

    loop {
        let channel = Arc::new(WhatsAppWebChannel::new(
            wa.session_path.clone().unwrap_or_default(),
            wa.pair_phone.clone(),
            wa.pair_code.clone(),
            wa.allowed_numbers.clone(),
            wa.mention_only,
            wa.mode.clone(),
            wa.dm_policy.clone(),
            wa.group_policy.clone(),
            wa.self_chat_mode,
        ));

        let mut events = channel.subscribe_pairing_events();
        let listen_channel = channel.clone();
        let handle = tokio::spawn(async move {
            let (tx, _rx) = tokio::sync::mpsc::channel(1);
            let _ = listen_channel.listen(tx).await;
        });

        let outcome = wait_for_pairing_outcome(&mut events, ui).await;

        match outcome {
            PairingOutcome::Connected => {
                ui.status("WhatsApp connected successfully.");
                ui.note(
                    "Restart the brai service now (e.g. `systemctl restart brai`) \
                    to start using WhatsApp.",
                );
                return Ok(());
            }
            PairingOutcome::Failed(reason) => {
                handle.abort();
                ui.warn(&format!("WhatsApp pairing failed: {reason}"));
            }
            PairingOutcome::TimedOut => {
                handle.abort();
                ui.warn("Pair code expired or no response received.");
            }
        }

        match ui.confirm("Try WhatsApp pairing again?", true).await? {
            brai_config::traits::Answer::Value(true) => continue,
            _ => {
                ui.note("You can retry later by restarting the brai service manually.");
                return Ok(());
            }
        }
    }
}

#[cfg(not(feature = "whatsapp-web"))]
pub async fn run_whatsapp_pairing_step(
    _wa: &brai_config::schema::WhatsAppConfig,
    ui: &mut dyn brai_config::traits::OnboardUi,
) -> anyhow::Result<()> {
    ui.warn("WhatsApp Web support was not compiled into this build; skipping pairing step.");
    Ok(())
}

#[cfg(feature = "whatsapp-web")]
enum PairingOutcome {
    Connected,
    Failed(String),
    TimedOut,
}

/// Wait for the pair-code to arrive (printing it via `ui`), then wait again
/// for `Connected`. Each wait has its own 60-second window.
#[cfg(feature = "whatsapp-web")]
async fn wait_for_pairing_outcome(
    events: &mut tokio::sync::broadcast::Receiver<brai_channels::whatsapp_web::PairingEvent>,
    ui: &mut dyn brai_config::traits::OnboardUi,
) -> PairingOutcome {
    use brai_channels::whatsapp_web::PairingEvent;
    use std::time::Duration;

    loop {
        match tokio::time::timeout(Duration::from_secs(60), events.recv()).await {
            Ok(Ok(PairingEvent::Code(code))) => {
                ui.note(&format!(
                    "WhatsApp pair code: {code}\n\n\
                    On the dedicated WhatsApp number's phone: Settings → Linked Devices \
                    → Link a Device → Link with phone number instead → enter this code.",
                ));
                // Reset the window: keep looping to wait for Connected next.
                continue;
            }
            Ok(Ok(PairingEvent::Connected)) => return PairingOutcome::Connected,
            Ok(Ok(PairingEvent::LoggedOut)) => {
                return PairingOutcome::Failed("session was logged out".to_string());
            }
            Ok(Ok(PairingEvent::StreamError(e))) => return PairingOutcome::Failed(e),
            Ok(Err(_lagged)) => {
                // Broadcast receiver fell behind (extremely unlikely with a
                // capacity-8 channel and this few events) — keep waiting
                // rather than treating a lag as a hard failure.
                continue;
            }
            Err(_timeout) => return PairingOutcome::TimedOut,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_leading_plus_and_keeps_digits() {
        assert_eq!(normalize_master_identity("+15550001111"), Some("+15550001111".to_string()));
    }

    #[test]
    fn adds_plus_when_missing() {
        assert_eq!(normalize_master_identity("15550001111"), Some("+15550001111".to_string()));
    }

    #[test]
    fn strips_spaces_and_dashes() {
        assert_eq!(normalize_master_identity("+1 555-000-1111"), Some("+15550001111".to_string()));
    }

    #[test]
    fn empty_input_returns_none() {
        assert_eq!(normalize_master_identity(""), None);
    }

    #[test]
    fn non_digit_input_returns_none() {
        assert_eq!(normalize_master_identity("abc"), None);
    }

    #[test]
    fn already_normalized_is_idempotent() {
        let once = normalize_master_identity("15550001111").unwrap();
        let twice = normalize_master_identity(&once).unwrap();
        assert_eq!(once, twice);
    }
}
