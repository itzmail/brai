use std::time::Duration;

use async_trait::async_trait;
use brai_api::tool::{Tool, ToolResult};
use serde_json::json;

const UPSTREAM_OWNER: &str = "zeroclaw-labs";
const UPSTREAM_REPO: &str = "zeroclaw";
const GITHUB_API: &str = "https://api.github.com";

/// Fetches latest zeroclaw upstream version and compares with brai's current base.
/// Uses GitHub REST API (no auth required — public repo, 60 req/hr limit).
pub struct ZeroclawUpstreamTool {
    http: reqwest::Client,
}

impl ZeroclawUpstreamTool {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("brai-agent/1.0")
            .build()
            .expect("failed to build HTTP client");
        Self { http }
    }

    async fn fetch_latest_tag(&self) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct Tag {
            name: String,
        }

        let url = format!("{GITHUB_API}/repos/{UPSTREAM_OWNER}/{UPSTREAM_REPO}/tags?per_page=1");
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("GitHub tags API failed: {text}");
        }
        let tags: Vec<Tag> = resp.json().await?;
        tags.into_iter()
            .next()
            .map(|t| t.name)
            .ok_or_else(|| anyhow::anyhow!("No tags found in upstream repo"))
    }

    async fn fetch_release_notes(&self, tag: &str) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct Release {
            body: Option<String>,
            name: Option<String>,
            published_at: Option<String>,
        }

        let url = format!(
            "{GITHUB_API}/repos/{UPSTREAM_OWNER}/{UPSTREAM_REPO}/releases/tags/{tag}"
        );
        let resp = self.http.get(&url).send().await?;
        if resp.status() == 404 {
            return Ok("No release notes available for this tag.".into());
        }
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("GitHub release API failed: {text}");
        }
        let release: Release = resp.json().await?;
        let name = release.name.unwrap_or_else(|| tag.to_string());
        let date = release.published_at.unwrap_or_default();
        let body = release.body.unwrap_or_else(|| "No description.".into());
        Ok(format!("Release: {name}\nDate: {date}\n\n{body}"))
    }

    async fn fetch_compare_commits(&self, base: &str, head: &str) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct Compare {
            commits: Vec<Commit>,
        }
        #[derive(serde::Deserialize)]
        struct Commit {
            commit: CommitDetail,
        }
        #[derive(serde::Deserialize)]
        struct CommitDetail {
            message: String,
        }

        let url = format!(
            "{GITHUB_API}/repos/{UPSTREAM_OWNER}/{UPSTREAM_REPO}/compare/{base}...{head}"
        );
        let resp = self.http.get(&url).send().await?;
        if resp.status() == 404 {
            return Ok(format!("Cannot compare {base}...{head} — tag may not exist yet."));
        }
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("GitHub compare API failed: {text}");
        }
        let compare: Compare = resp.json().await?;
        if compare.commits.is_empty() {
            return Ok("No new commits since last tracked version.".into());
        }
        let lines: Vec<String> = compare
            .commits
            .iter()
            .map(|c| {
                let first_line = c.commit.message.lines().next().unwrap_or("").to_string();
                format!("- {first_line}")
            })
            .collect();
        Ok(lines.join("\n"))
    }
}

impl Default for ZeroclawUpstreamTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ZeroclawUpstreamTool {
    fn name(&self) -> &str {
        "zeroclaw_check_updates"
    }

    fn description(&self) -> &str {
        "Check for new releases in the upstream zeroclaw repository and get a summary of changes. \
         Compares the current brai base version against the latest upstream tag, fetches release \
         notes and commit list, then recommends which changes are feasible to implement in brai \
         given its constraints (Telegram-only, low-resource VPS, no browser/WASM/TUI/hardware)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "current_version": {
                    "type": "string",
                    "description": "The zeroclaw version brai is currently based on (e.g. 'v0.7.5'). \
                                    If omitted, defaults to brai's workspace version."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let current_version = args
            .get("current_version")
            .and_then(|v| v.as_str())
            .unwrap_or(env!("CARGO_PKG_VERSION"));

        // Normalize: ensure version has 'v' prefix for tag comparison
        let base_tag = if current_version.starts_with('v') {
            current_version.to_string()
        } else {
            format!("v{current_version}")
        };

        let latest_tag = match self.fetch_latest_tag().await {
            Ok(t) => t,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to fetch upstream tags: {e}")),
                })
            }
        };

        if latest_tag == base_tag {
            return Ok(ToolResult {
                success: true,
                output: format!(
                    "Brai is up to date with upstream zeroclaw.\nCurrent: {base_tag}\nLatest: {latest_tag}"
                ),
                error: None,
            });
        }

        // Fetch release notes and commit diff in parallel
        let (release_notes, commits) = tokio::join!(
            self.fetch_release_notes(&latest_tag),
            self.fetch_compare_commits(&base_tag, &latest_tag),
        );

        let release_notes = release_notes.unwrap_or_else(|e| format!("[release notes error: {e}]"));
        let commits = commits.unwrap_or_else(|e| format!("[commit diff error: {e}]"));

        let output = format!(
            "=== ZEROCLAW UPSTREAM UPDATE ===\n\
             Base (brai): {base_tag}\n\
             Latest upstream: {latest_tag}\n\
             \n\
             --- RELEASE NOTES ---\n\
             {release_notes}\n\
             \n\
             --- NEW COMMITS ---\n\
             {commits}\n\
             \n\
             --- BRAI FEASIBILITY FILTER ---\n\
             Review the commits above and assess feasibility for brai given these constraints:\n\
             - SKIP: browser, WASM plugins, TUI/onboarding, hardware/peripheral, desktop, Tauri, Discord/Nostr/WebSocket channels\n\
             - SKIP: features requiring >50MB RAM overhead or heavy dependencies\n\
             - CANDIDATE: new tool types (HTTP-based, file, memory, LLM task)\n\
             - CANDIDATE: performance and memory efficiency improvements\n\
             - CANDIDATE: Telegram channel improvements\n\
             - CANDIDATE: security, config, provider improvements\n\
             - CANDIDATE: bug fixes in core agent loop, tool execution, cron/SOP\n\
             \n\
             Evaluate each commit and recommend: IMPLEMENT / SKIP / REVIEW_FIRST"
        );

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        let tool = ZeroclawUpstreamTool::new();
        assert_eq!(tool.name(), "zeroclaw_check_updates");
    }

    #[test]
    fn parameters_schema_valid() {
        let tool = ZeroclawUpstreamTool::new();
        let schema = tool.parameters_schema();
        assert!(schema.get("properties").is_some());
    }

    #[test]
    fn version_prefix_normalization() {
        // Verify both "0.7.5" and "v0.7.5" produce same base_tag
        let with_v = "v0.7.5".to_string();
        let without_v = "0.7.5";
        let normalized = if without_v.starts_with('v') {
            without_v.to_string()
        } else {
            format!("v{without_v}")
        };
        assert_eq!(with_v, normalized);
    }
}
