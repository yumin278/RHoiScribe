# MCP Client Setup

RHoiScribe is a local MCP server launched through stdio. It is intended for Codex, Claude Code, and other MCP-compatible clients that can start a local stdio command.

Build the release binary first:

```powershell
cargo build --release
```

Use placeholders in docs and committed examples. Replace them only in your private client configuration:

- `<ABSOLUTE_PATH_TO_RHOISCRIBE>`: absolute path to this repository on the user's machine.
- `<MOD_OUTPUT_ROOT>`: absolute path to a HOI4 mod folder when a generation tool writes files.

Default binary paths:

- Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe`
- Linux: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe`
- macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe`

## Codex

Add RHoiScribe to the Codex MCP server configuration using the release binary as the command.

```toml
[mcp_servers.rhoiscribe]
command = "<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe"
args = []
```

For Windows:

```toml
[mcp_servers.rhoiscribe]
command = "<ABSOLUTE_PATH_TO_RHOISCRIBE>\\target\\release\\rhoiscribe.exe"
args = []
```

If your Codex surface uses a different config location, keep the same server name, command path, and empty `args` shape.

## Claude Code

Claude Code can register local stdio MCP servers from its MCP configuration or CLI. Use the release binary as the command.

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe",
      "args": []
    }
  }
}
```

For Windows:

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<ABSOLUTE_PATH_TO_RHOISCRIBE>\\target\\release\\rhoiscribe.exe",
      "args": []
    }
  }
}
```

CLI-style registration can use the same command path:

```powershell
claude mcp add rhoiscribe -- <ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

## Generic MCP JSON

Many MCP clients accept a server map with `command` and `args` fields:

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe",
      "args": []
    }
  }
}
```

Windows clients usually need the `.exe` path and escaped backslashes in JSON:

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<ABSOLUTE_PATH_TO_RHOISCRIBE>\\target\\release\\rhoiscribe.exe",
      "args": []
    }
  }
}
```

## Runtime Behavior

- Transport: stdio.
- Network: no runtime network access is required.
- Prompts: available through `prompts/list` and `prompts/get`.
- Resources: available through `resources/list` and `resources/read`.
- Tools: available through `tools/list` and `tools/call`.
- Write mode: generation tools require `dry_run = false` and `output_root = "<MOD_OUTPUT_ROOT>"`.

## Smoke Test

After adding the server to a client, ask the client to list MCP resources and read:

```text
rhoiscribe://hoi4/knowledge/catalog
```

Then call `generate_localisation_batch` with `dry_run = true` before allowing write mode. The returned file path should stay under a valid `localisation/<language>/` tree, including nested subdirectories when they match the user's mod, and the encoding should be `utf-8-bom`.
