//------------------------------------------------------------------------------------
// main.rs -- Part of RHoiScribe
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match rhoiscribe::cli::parse_args(std::env::args())? {
        rhoiscribe::cli::CliCommand::Serve => rhoiscribe::server::run_stdio_server().await,
        rhoiscribe::cli::CliCommand::Help => {
            print!("{}", rhoiscribe::cli::help_text());
            Ok(())
        }
        rhoiscribe::cli::CliCommand::Version => {
            println!("{}", rhoiscribe::cli::version_text());
            Ok(())
        }
        rhoiscribe::cli::CliCommand::PrintCommand => {
            println!("{}", rhoiscribe::cli::command_path_for_mcp_json()?);
            Ok(())
        }
        rhoiscribe::cli::CliCommand::Logs(filters) => run_log_query(filters),
        rhoiscribe::cli::CliCommand::ExportLogs(export) => run_log_export(export),
        rhoiscribe::cli::CliCommand::Skill(command) => {
            println!("{}", rhoiscribe::skill::execute_skill_command(command)?);
            Ok(())
        }
    }
}

fn run_log_query(filters: rhoiscribe::cli::LogFilterCliArgs) -> anyhow::Result<()> {
    let result = rhoiscribe::tools::ToolEngine::query_tool_logs(log_query_request(filters))?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn run_log_export(export: rhoiscribe::cli::LogExportCliArgs) -> anyhow::Result<()> {
    let result = rhoiscribe::tools::ToolEngine::export_tool_logs(log_export_request(export))?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn log_query_request(
    filters: rhoiscribe::cli::LogFilterCliArgs,
) -> rhoiscribe::tools::ToolLogQueryRequest {
    rhoiscribe::tools::ToolLogQueryRequest {
        store_path: filters.store_path,
        mod_root: filters.mod_root,
        tool_name: filters.tool_name,
        success: filters.success,
        since_unix_seconds: filters.since_unix_seconds,
        until_unix_seconds: filters.until_unix_seconds,
        text_query: filters.text_query,
        pattern: filters.pattern,
        limit: filters.limit,
    }
}

fn log_export_request(
    export: rhoiscribe::cli::LogExportCliArgs,
) -> rhoiscribe::tools::ToolLogExportRequest {
    let filters = export.filters;
    rhoiscribe::tools::ToolLogExportRequest {
        store_path: filters.store_path,
        output_path: export.output_path,
        mod_root: filters.mod_root,
        tool_name: filters.tool_name,
        success: filters.success,
        since_unix_seconds: filters.since_unix_seconds,
        until_unix_seconds: filters.until_unix_seconds,
        text_query: filters.text_query,
        pattern: filters.pattern,
        limit: filters.limit,
    }
}
