//! Channel-agnostic Contact Approval Gate.
//!
//! Tracks pending/approved/denied status per (channel, identity) pair in a
//! small SQLite table, so an unknown contact must be approved by the
//! channel's configured master identity before brai will talk to them.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateStatus {
    Pending,
    Approved,
    Denied,
}

impl GateStatus {
    fn as_str(self) -> &'static str {
        match self {
            GateStatus::Pending => "pending",
            GateStatus::Approved => "approved",
            GateStatus::Denied => "denied",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(GateStatus::Pending),
            "approved" => Some(GateStatus::Approved),
            "denied" => Some(GateStatus::Denied),
            _ => None,
        }
    }
}

pub struct ContactGate {
    conn: Mutex<Connection>,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl ContactGate {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory for {}", db_path.display()))?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open contact gate DB: {}", db_path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS contact_gate (
                channel      TEXT NOT NULL,
                identity     TEXT NOT NULL,
                status       TEXT NOT NULL,
                requested_at INTEGER NOT NULL,
                decided_at   INTEGER,
                PRIMARY KEY (channel, identity)
             );",
        )
        .context("Failed to initialize contact_gate schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
            db_path: db_path.to_path_buf(),
        })
    }

    pub fn status(&self, channel: &str, identity: &str) -> Option<GateStatus> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT status FROM contact_gate WHERE channel = ?1 AND identity = ?2",
            params![channel, identity],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .and_then(|s| GateStatus::from_str(&s))
    }

    pub fn mark_pending(&self, channel: &str, identity: &str) -> Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR IGNORE INTO contact_gate (channel, identity, status, requested_at)
             VALUES (?1, ?2, 'pending', ?3)",
            params![channel, identity, now],
        )
        .context("Failed to insert pending contact_gate row")?;
        Ok(())
    }

    pub fn approve(&self, channel: &str, identity: &str) -> Result<bool> {
        self.set_status(channel, identity, GateStatus::Approved)
    }

    pub fn deny(&self, channel: &str, identity: &str) -> Result<bool> {
        self.set_status(channel, identity, GateStatus::Denied)
    }

    fn set_status(&self, channel: &str, identity: &str, status: GateStatus) -> Result<bool> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        let changed = conn
            .execute(
                "INSERT INTO contact_gate (channel, identity, status, requested_at, decided_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(channel, identity) DO UPDATE SET
                    status = excluded.status,
                    decided_at = excluded.decided_at",
                params![channel, identity, status.as_str(), now],
            )
            .context("Failed to set contact_gate status")?;
        Ok(changed > 0)
    }

    pub fn revoke(&self, channel: &str, identity: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "DELETE FROM contact_gate WHERE channel = ?1 AND identity = ?2",
                params![channel, identity],
            )
            .context("Failed to revoke contact_gate row")?;
        Ok(changed > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open_test_gate() -> (TempDir, ContactGate) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("contact_gate.db");
        let gate = ContactGate::open(&db_path).unwrap();
        (tmp, gate)
    }

    #[test]
    fn unknown_identity_has_no_status() {
        let (_tmp, gate) = open_test_gate();
        assert_eq!(gate.status("whatsapp", "+1234567890"), None);
    }

    #[test]
    fn mark_pending_then_status_is_pending() {
        let (_tmp, gate) = open_test_gate();
        gate.mark_pending("whatsapp", "+1234567890").unwrap();
        assert_eq!(
            gate.status("whatsapp", "+1234567890"),
            Some(GateStatus::Pending)
        );
    }

    #[test]
    fn mark_pending_is_idempotent() {
        let (_tmp, gate) = open_test_gate();
        gate.mark_pending("whatsapp", "+1234567890").unwrap();
        gate.approve("whatsapp", "+1234567890").unwrap();
        // Re-marking pending after approval must NOT reset an approved contact.
        gate.mark_pending("whatsapp", "+1234567890").unwrap();
        assert_eq!(
            gate.status("whatsapp", "+1234567890"),
            Some(GateStatus::Approved)
        );
    }

    #[test]
    fn approve_transitions_pending_to_approved() {
        let (_tmp, gate) = open_test_gate();
        gate.mark_pending("whatsapp", "+1234567890").unwrap();
        assert!(gate.approve("whatsapp", "+1234567890").unwrap());
        assert_eq!(
            gate.status("whatsapp", "+1234567890"),
            Some(GateStatus::Approved)
        );
    }

    #[test]
    fn deny_transitions_pending_to_denied() {
        let (_tmp, gate) = open_test_gate();
        gate.mark_pending("whatsapp", "+1234567890").unwrap();
        assert!(gate.deny("whatsapp", "+1234567890").unwrap());
        assert_eq!(
            gate.status("whatsapp", "+1234567890"),
            Some(GateStatus::Denied)
        );
    }

    #[test]
    fn revoke_removes_row_entirely() {
        let (_tmp, gate) = open_test_gate();
        gate.mark_pending("whatsapp", "+1234567890").unwrap();
        gate.approve("whatsapp", "+1234567890").unwrap();
        assert!(gate.revoke("whatsapp", "+1234567890").unwrap());
        assert_eq!(gate.status("whatsapp", "+1234567890"), None);
    }

    #[test]
    fn revoke_nonexistent_returns_false() {
        let (_tmp, gate) = open_test_gate();
        assert!(!gate.revoke("whatsapp", "+1234567890").unwrap());
    }

    #[test]
    fn status_is_scoped_per_channel() {
        let (_tmp, gate) = open_test_gate();
        gate.approve("whatsapp", "+1234567890").unwrap();
        assert_eq!(gate.status("telegram", "+1234567890"), None);
        assert_eq!(
            gate.status("whatsapp", "+1234567890"),
            Some(GateStatus::Approved)
        );
    }

    #[test]
    fn persists_across_reopen() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("contact_gate.db");
        {
            let gate = ContactGate::open(&db_path).unwrap();
            gate.approve("whatsapp", "+1234567890").unwrap();
        }
        let gate2 = ContactGate::open(&db_path).unwrap();
        assert_eq!(
            gate2.status("whatsapp", "+1234567890"),
            Some(GateStatus::Approved)
        );
    }
}
