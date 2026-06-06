# MCP Client Setup

RHoiScribe is a local MCP server launched through stdio. Build the release binary first:

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

The binary supports:

```powershell
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe --help
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe --version
```

## Roo Code

Add RHoiScribe as a stdio MCP server in Roo Code's MCP settings. Use the absolute path to the release binary.

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

## Codex

Add RHoiScribe to the Codex MCP server configuration for the repository or user config, using the local binary as the command.

```toml
[mcp_servers.rhoiscribe]
command = "<ABSOLUTE_PATH_TO_RHOISCRIBE>\\target\\release\\rhoiscribe.exe"
args = []
```

If your Codex surface expects a different config file or server table name, keep the same command value and stdio behavior.

## Claude Code

Claude Code can register local stdio MCP servers from the CLI. Use an absolute path to the release binary.

```powershell
claude mcp add rhoiscribe -- <ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

## Generic MCP JSON

Many MCP-compatible clients accept this shape:

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

Then call `generate_localisation_batch` with `dry_run = true` before allowing write mode. The returned file path should stay under `localisation/<language>/` and the encoding should be `utf-8-bom`.
