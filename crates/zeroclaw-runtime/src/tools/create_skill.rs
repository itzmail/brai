use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use brai_api::tool::{Tool, ToolResult};

use crate::skills::audit;
use crate::skills::creator::{SkillCreator, toml_escape};

/// Tool for creating new skill definitions on demand from structured LLM input.
pub struct CreateSkillTool {
    workspace_dir: PathBuf,
}

impl CreateSkillTool {
    pub fn new(workspace_dir: PathBuf) -> Self {
        Self { workspace_dir }
    }
}

#[async_trait]
impl Tool for CreateSkillTool {
    fn name(&self) -> &str {
        "create_skill"
    }

    fn description(&self) -> &str {
        "Create a new reusable skill definition and save it to the workspace. \
         The skill becomes available immediately after creation. \
         Use this when the user asks you to remember or codify a workflow, \
         or when building a new capability (e.g. a security check, a deploy script). \
         Always clarify the skill's name, description, and steps before calling this tool."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Skill name: lowercase alphanumeric and hyphens only, max 64 chars. E.g. 'tls-check', 'log-audit'."
                },
                "description": {
                    "type": "string",
                    "description": "One-sentence description of what the skill does."
                },
                "prompts": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional list of instruction strings injected into the agent context when the skill is active."
                },
                "tools": {
                    "type": "array",
                    "description": "Optional list of shell/http tool definitions bundled with this skill.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name":        {"type": "string"},
                            "description": {"type": "string"},
                            "kind":        {"type": "string", "enum": ["shell", "http", "script"]},
                            "command":     {"type": "string"}
                        },
                        "required": ["name", "description", "kind", "command"]
                    }
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional tags for categorization."
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Overwrite if a skill with this name already exists. Default false."
                }
            },
            "required": ["name", "description"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let Some(name) = args
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Missing required parameter 'name'.".into()),
            });
        };

        let Some(description) = args
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Missing required parameter 'description'.".into()),
            });
        };

        let slug = SkillCreator::generate_slug(name);
        if !SkillCreator::validate_slug(&slug) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Invalid skill name '{name}'. Use lowercase alphanumeric and hyphens only, max 64 chars, no leading/trailing hyphens."
                )),
            });
        }

        let overwrite = args.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);

        let skill_dir = crate::skills::skills_dir(&self.workspace_dir).join(&slug);

        if skill_dir.exists() && !overwrite {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Skill '{slug}' already exists. Pass overwrite=true to replace it."
                )),
            });
        }

        let toml_content = build_skill_toml(&slug, description, &args);

        tokio::fs::create_dir_all(&skill_dir).await.map_err(|e| {
            anyhow::anyhow!("Failed to create skill directory {}: {e}", skill_dir.display())
        })?;

        let toml_path = skill_dir.join("SKILL.toml");
        tokio::fs::write(&toml_path, toml_content.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write {}: {e}", toml_path.display()))?;

        match audit::audit_skill_directory(&skill_dir) {
            Ok(report) if report.is_clean() => {}
            Ok(report) => {
                let _ = tokio::fs::remove_dir_all(&skill_dir).await;
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "Skill failed security audit and was not saved: {}",
                        report.summary()
                    )),
                });
            }
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&skill_dir).await;
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Skill audit error: {e}")),
                });
            }
        }

        Ok(ToolResult {
            success: true,
            output: format!(
                "Skill '{slug}' created at {}. It will be available in the next agent turn.",
                toml_path.display()
            ),
            error: None,
        })
    }
}

fn build_skill_toml(slug: &str, description: &str, args: &serde_json::Value) -> String {
    use std::fmt::Write;
    let mut toml = String::new();

    toml.push_str("[skill]\n");
    let _ = writeln!(toml, "name = {}", toml_escape(slug));
    let _ = writeln!(toml, "description = {}", toml_escape(description));
    toml.push_str("version = \"0.1.0\"\n");
    toml.push_str("author = \"brai\"\n");

    if let Some(tags) = args.get("tags").and_then(|v| v.as_array()) {
        let tag_strs: Vec<String> = tags
            .iter()
            .filter_map(|t| t.as_str())
            .map(|t| format!("\"{}\"", t.replace('"', "\\\"")))
            .collect();
        if !tag_strs.is_empty() {
            let _ = writeln!(toml, "tags = [{}]", tag_strs.join(", "));
        }
    }

    if let Some(prompts) = args.get("prompts").and_then(|v| v.as_array()) {
        for prompt in prompts.iter().filter_map(|p| p.as_str()) {
            let _ = writeln!(toml, "\n[[prompts]]\ntext = {}", toml_escape(prompt));
        }
    }

    if let Some(tools) = args.get("tools").and_then(|v| v.as_array()) {
        for tool in tools {
            let tool_name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let tool_desc = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let tool_kind = tool.get("kind").and_then(|v| v.as_str()).unwrap_or("shell");
            let tool_cmd = tool.get("command").and_then(|v| v.as_str()).unwrap_or("");

            if tool_name.is_empty() || tool_cmd.is_empty() {
                continue;
            }

            toml.push('\n');
            toml.push_str("[[tools]]\n");
            let _ = writeln!(toml, "name = {}", toml_escape(tool_name));
            let _ = writeln!(toml, "description = {}", toml_escape(tool_desc));
            let _ = writeln!(toml, "kind = {}", toml_escape(tool_kind));
            let _ = writeln!(toml, "command = {}", toml_escape(tool_cmd));
        }
    }

    toml
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tool(tmp: &TempDir) -> CreateSkillTool {
        let workspace = tmp.path().join("workspace");
        CreateSkillTool::new(workspace)
    }

    #[tokio::test]
    async fn creates_valid_skill() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);

        let result = tool
            .execute(json!({
                "name": "tls-check",
                "description": "Check TLS certificate expiry for a domain"
            }))
            .await
            .unwrap();

        assert!(result.success, "expected success, got: {:?}", result.error);
        let skill_path = tmp
            .path()
            .join("workspace/skills/tls-check/SKILL.toml");
        assert!(skill_path.exists(), "SKILL.toml not written");
        let content = std::fs::read_to_string(&skill_path).unwrap();
        assert!(content.contains("tls-check"));
        assert!(content.contains("Check TLS certificate expiry"));
        assert!(content.contains("author = \"brai\""));
    }

    #[tokio::test]
    async fn skill_readable_by_load_skills() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);

        tool.execute(json!({
            "name": "dns-recon",
            "description": "DNS enumeration skill"
        }))
        .await
        .unwrap();

        let workspace = tmp.path().join("workspace");
        let skills = crate::skills::load_skills(&workspace);
        assert!(
            skills.iter().any(|s| s.name == "dns-recon"),
            "skill not found by load_skills"
        );
    }

    #[tokio::test]
    async fn rejects_duplicate_without_overwrite() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);

        tool.execute(json!({"name": "my-skill", "description": "first"}))
            .await
            .unwrap();

        let result = tool
            .execute(json!({"name": "my-skill", "description": "second"}))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.as_deref().unwrap_or("").contains("already exists"));
    }

    #[tokio::test]
    async fn overwrites_when_flag_set() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);

        tool.execute(json!({"name": "my-skill", "description": "first"}))
            .await
            .unwrap();

        let result = tool
            .execute(json!({"name": "my-skill", "description": "updated", "overwrite": true}))
            .await
            .unwrap();

        assert!(result.success);
        let content = std::fs::read_to_string(
            tmp.path().join("workspace/skills/my-skill/SKILL.toml"),
        )
        .unwrap();
        assert!(content.contains("updated"));
    }

    #[tokio::test]
    async fn rejects_invalid_slug() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);

        let result = tool
            .execute(json!({"name": "", "description": "no name"}))
            .await
            .unwrap();

        assert!(!result.success);
    }

    #[tokio::test]
    async fn includes_tools_in_toml() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);

        let result = tool
            .execute(json!({
                "name": "tls-check",
                "description": "TLS check skill",
                "tools": [
                    {
                        "name": "check_cert",
                        "description": "Check cert expiry",
                        "kind": "shell",
                        "command": "openssl s_client -connect {domain}:443 </dev/null 2>/dev/null | openssl x509 -noout -dates"
                    }
                ]
            }))
            .await
            .unwrap();

        assert!(result.success, "{:?}", result.error);
        let content = std::fs::read_to_string(
            tmp.path().join("workspace/skills/tls-check/SKILL.toml"),
        )
        .unwrap();
        assert!(content.contains("[[tools]]"));
        assert!(content.contains("check_cert"));
        assert!(content.contains("openssl s_client"));
    }
}
