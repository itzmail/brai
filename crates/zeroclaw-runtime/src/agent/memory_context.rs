//! Shared TOON encoding for memory context injected into agent prompts.
//!
//! TOON (Token-Oriented Object Notation) renders a uniform array of records
//! as a single tabular block, which costs fewer tokens than one bullet line
//! per entry once there is more than a couple of rows.

use brai_memory::{MEMORY_CONTEXT_CLOSE, MEMORY_CONTEXT_OPEN};
use serde::Serialize;

/// A single memory entry reduced to the fields worth showing the model.
#[derive(Serialize)]
pub struct MemoryContextRow {
    pub key: String,
    pub content: String,
    pub score: Option<f64>,
}

/// Encode filtered memory rows into a `[Memory context]` TOON block.
/// Returns an empty string when there are no rows to include.
pub fn encode_memory_context(rows: &[MemoryContextRow]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let Ok(toon) = toon_format::encode_default(&rows) else {
        return String::new();
    };

    format!("{MEMORY_CONTEXT_OPEN}\n{toon}\n{MEMORY_CONTEXT_CLOSE}\n\n")
}
