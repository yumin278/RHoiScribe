//------------------------------------------------------------------------------------
// server.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use std::{future, future::Future};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult,
        Implementation, ListPromptsResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, ServerCapabilities,
        ServerInfo,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};

use crate::{prompts::PromptCatalog, resources::ResourceCatalog, tools::ToolCatalog};

pub const SERVER_NAME: &str = "rhoiscribe";
pub const SERVER_TITLE: &str = "RHoiScribe";
pub const SERVER_INSTRUCTIONS: &str = "RHoiScribe provides local MCP prompts, resources, and batch tools for HOI4 Modding agents. Read bundled resources before web search, use validate_hoi4_project before finishing any file-changing HOI4 task, and run repair_hoi4_project dry_run=true after file changes so encoding, formatting, and media conventions are normalized by the repair tool instead of manual per-file fixes.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerMetadata {
    pub name: &'static str,
    pub title: &'static str,
    pub version: &'static str,
    pub instructions: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct RhoiScribeServer;

impl RhoiScribeServer {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata(&self) -> ServerMetadata {
        ServerMetadata {
            name: SERVER_NAME,
            title: SERVER_TITLE,
            version: env!("CARGO_PKG_VERSION"),
            instructions: SERVER_INSTRUCTIONS,
        }
    }

    pub fn server_info(&self) -> ServerInfo {
        let metadata = self.metadata();

        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(
            Implementation::new(metadata.name, metadata.version)
                .with_title(metadata.title)
                .with_description("Local MCP server for HOI4 Modding agent workflows"),
        )
        .with_instructions(metadata.instructions)
    }
}

impl ServerHandler for RhoiScribeServer {
    fn get_info(&self) -> ServerInfo {
        self.server_info()
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + MaybeSendFuture + '_ {
        future::ready(Ok(ListPromptsResult::with_all_items(
            PromptCatalog::builtin().to_mcp_prompts(),
        )))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + MaybeSendFuture + '_ {
        let arguments = request.arguments.unwrap_or_default();
        future::ready(
            PromptCatalog::builtin()
                .render(&request.name, &arguments)
                .map_err(|error| McpError::invalid_params(error.to_string(), None)),
        )
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + MaybeSendFuture + '_ {
        future::ready(
            ResourceCatalog::load_embedded()
                .map(|catalog| ListResourcesResult::with_all_items(catalog.to_mcp_resources()))
                .map_err(|error| McpError::internal_error(error.to_string(), None)),
        )
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + MaybeSendFuture + '_ {
        future::ready(
            ResourceCatalog::load_embedded()
                .map_err(|error| McpError::internal_error(error.to_string(), None))
                .and_then(|catalog| {
                    catalog
                        .read_mcp_resource(&request.uri)
                        .map_err(|error| McpError::invalid_params(error.to_string(), None))
                }),
        )
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + MaybeSendFuture + '_ {
        future::ready(Ok(ListToolsResult::with_all_items(
            ToolCatalog::builtin().to_mcp_tools(),
        )))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + MaybeSendFuture + '_ {
        let arguments = request.arguments.unwrap_or_default();
        future::ready(
            ToolCatalog::builtin()
                .call(&request.name, arguments)
                .map_err(|error| McpError::invalid_params(error.to_string(), None)),
        )
    }
}

pub async fn run_stdio_server() -> anyhow::Result<()> {
    RhoiScribeServer::new()
        .serve(stdio())
        .await?
        .waiting()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SERVER_INSTRUCTIONS;

    #[test]
    fn server_instructions_require_delivery_validation_and_repair() {
        assert!(SERVER_INSTRUCTIONS.contains("validate_hoi4_project before finishing"));
        assert!(SERVER_INSTRUCTIONS.contains("repair_hoi4_project dry_run=true"));
        assert!(SERVER_INSTRUCTIONS.contains("instead of manual per-file fixes"));
    }
}
