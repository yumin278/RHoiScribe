use std::{error::Error, fmt};

use rmcp::model::JsonObject;
use serde_json::{Map, Value, json};

use crate::{prompts::PromptCatalog, resources::ResourceCatalog, tools::ToolCatalog};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillCommand {
    ListTools,
    ListResources,
    ListPrompts,
    ReadResource {
        uri: String,
    },
    GetPrompt {
        name: String,
        arguments_json: String,
    },
    CallTool {
        name: String,
        arguments_json: String,
    },
}

#[derive(Debug)]
pub enum SkillError {
    InvalidJson {
        command: &'static str,
        source: serde_json::Error,
    },
    InvalidArguments {
        command: &'static str,
    },
    Prompt(String),
    Resource(String),
    Tool(String),
    Serialize(serde_json::Error),
}

pub fn execute_skill_command(command: SkillCommand) -> Result<String, SkillError> {
    match command {
        SkillCommand::ListTools => serialize(json!({
            "tools": ToolCatalog::builtin().to_mcp_tools()
        })),
        SkillCommand::ListResources => {
            let catalog = ResourceCatalog::load_embedded()
                .map_err(|error| SkillError::Resource(error.to_string()))?;
            serialize(json!({
                "resources": catalog.to_mcp_resources()
            }))
        }
        SkillCommand::ListPrompts => serialize(json!({
            "prompts": PromptCatalog::builtin().to_mcp_prompts()
        })),
        SkillCommand::ReadResource { uri } => {
            let catalog = ResourceCatalog::load_embedded()
                .map_err(|error| SkillError::Resource(error.to_string()))?;
            let result = catalog
                .read_mcp_resource(&uri)
                .map_err(|error| SkillError::Resource(error.to_string()))?;
            serialize(result)
        }
        SkillCommand::GetPrompt {
            name,
            arguments_json,
        } => {
            let arguments = parse_object("get-prompt", &arguments_json)?;
            let result = PromptCatalog::builtin()
                .render(&name, &arguments)
                .map_err(|error| SkillError::Prompt(error.to_string()))?;
            serialize(result)
        }
        SkillCommand::CallTool {
            name,
            arguments_json,
        } => {
            let arguments = parse_object("call-tool", &arguments_json)?;
            let result = ToolCatalog::builtin()
                .call(&name, arguments)
                .map_err(|error| SkillError::Tool(error.to_string()))?;
            serialize(result)
        }
    }
}

impl fmt::Display for SkillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillError::InvalidJson { command, source } => {
                write!(
                    formatter,
                    "{} arguments must be valid JSON: {}",
                    command, source
                )
            }
            SkillError::InvalidArguments { command } => {
                write!(formatter, "{} arguments must be a JSON object", command)
            }
            SkillError::Prompt(error) => write!(formatter, "prompt error: {}", error),
            SkillError::Resource(error) => write!(formatter, "resource error: {}", error),
            SkillError::Tool(error) => write!(formatter, "tool error: {}", error),
            SkillError::Serialize(error) => write!(formatter, "serialization error: {}", error),
        }
    }
}

impl Error for SkillError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SkillError::InvalidJson { source, .. } => Some(source),
            SkillError::Serialize(error) => Some(error),
            _ => None,
        }
    }
}

fn parse_object(command: &'static str, json_text: &str) -> Result<JsonObject, SkillError> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|source| SkillError::InvalidJson { command, source })?;

    let Value::Object(object) = value else {
        return Err(SkillError::InvalidArguments { command });
    };

    Ok(Map::from_iter(object))
}

fn serialize<T>(value: T) -> Result<String, SkillError>
where
    T: serde::Serialize,
{
    serde_json::to_string_pretty(&value).map_err(SkillError::Serialize)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{SkillCommand, execute_skill_command};
    use crate::{resources::LATEST_UPDATE_URI, tools::ToolCatalog};

    #[test]
    fn list_tools_matches_mcp_catalog() {
        let output =
            execute_skill_command(SkillCommand::ListTools).expect("tools should serialize");
        let value = serde_json::from_str::<Value>(&output).expect("output should be json");
        let tools = value["tools"].as_array().expect("tools should be an array");

        assert_eq!(tools.len(), ToolCatalog::builtin().to_mcp_tools().len());
        assert!(
            tools
                .iter()
                .any(|tool| tool["name"] == "discover_hoi4_environment")
        );
    }

    #[test]
    fn reads_resource_and_renders_prompt() {
        let resource = execute_skill_command(SkillCommand::ReadResource {
            uri: LATEST_UPDATE_URI.to_string(),
        })
        .expect("resource should be readable");
        let prompt = execute_skill_command(SkillCommand::GetPrompt {
            name: "hoi4_mod_planner".to_string(),
            arguments_json: r#"{"request":"add an industrial focus"}"#.to_string(),
        })
        .expect("prompt should render");

        assert!(resource.contains("Hearts of Iron IV"));
        assert!(prompt.contains("add an industrial focus"));
    }

    #[test]
    fn calls_tool_with_json_arguments() {
        let output = execute_skill_command(SkillCommand::CallTool {
            name: "format_paradox_script".to_string(),
            arguments_json: r#"{"script":"focus={id=abc cost=10}"}"#.to_string(),
        })
        .expect("tool should run");
        let value = serde_json::from_str::<Value>(&output).expect("tool output should be json");

        assert!(
            value["structuredContent"]["formatted"]
                .as_str()
                .expect("formatted script should be present")
                .contains("focus = {")
        );
    }

    #[test]
    fn release_skill_document_mentions_direct_commands() {
        let skill = include_str!("../resources/skill/SKILL.md");

        assert!(skill.contains("--skill list-tools"));
        assert!(skill.contains("--skill list-resources"));
        assert!(skill.contains("--skill list-prompts"));
        assert!(skill.contains("--skill call-tool"));
        assert!(skill.contains("same directory"));
    }

    #[test]
    fn release_workflow_includes_skill_archives() {
        let workflow = include_str!("../.github/workflows/release-builds.yml");

        assert!(workflow.contains("rhoiscribe-skill-windows-x86_64.zip"));
        assert!(workflow.contains("rhoiscribe-skill-linux-x86_64.zip"));
        assert!(workflow.contains("rhoiscribe-skill-macos-universal.zip"));
        assert!(workflow.contains("resources/skill/SKILL.md"));
    }

    #[test]
    fn release_workflow_avoids_duplicate_body_title_and_cleans_old_releases() {
        let workflow = include_str!("../.github/workflows/release-builds.yml");

        assert!(!workflow.contains("echo \"## RHoiScribe $TAG_NAME\""));
        assert!(workflow.contains("Clean duplicate release body headings"));
        assert!(workflow.contains("gh release list"));
        assert!(workflow.contains(r#"\A## RHoiScribe v[0-9]"#));
    }
}
