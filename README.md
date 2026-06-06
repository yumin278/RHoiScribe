# RHoiScribe

RHoiScribe is a local Rust MCP server for HOI4 Modding agents. It bundles prompts, HOI4 knowledge resources, latest-update notes, and batch generation tools so AI agents can work from local context before writing game-readable mod files.

The project is intended for Roo Code, Codex, Claude Code, and other MCP-compatible clients that can launch a stdio server.

## What It Provides

- Built-in prompts for mod planning, script writing, localisation, GUI/scripted GUI work, and review.
- Local resources for the latest HOI4 update snapshot and a structured HOI4 modding knowledge catalog.
- Batch tools that generate common HOI4 content files and return structured previews before writing.
- Validation helpers for safe mod-relative paths and basic Paradox script formatting.
- Offline runtime behavior; the server does not need network access while serving prompts, resources, or tools.

The bundled knowledge catalog is not a verbatim mirror of any wiki. It is a structured reference layer designed for agents, with topic IDs, categories, file types, tags, syntax blocks, relationships, validation rules, and source references.

## Build

```powershell
cargo build --release
```

Use the absolute path to the built binary in MCP clients:

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

On Linux and macOS the binary path ends with:

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe
```

## Run

Run with no arguments to start the MCP server over stdio:

```powershell
.\target\release\rhoiscribe.exe
```

CLI flags:

```powershell
.\target\release\rhoiscribe.exe --help
.\target\release\rhoiscribe.exe --version
```

## MCP Usage

After a client starts RHoiScribe as a stdio MCP server, agents interact with it through normal MCP methods.

### Prompts

- `prompts/list` returns the built-in prompt names.
- `prompts/get` returns a selected prompt with arguments rendered by the client.

Current prompts:

- `hoi4_mod_planner`: plan a mod feature before writing files.
- `hoi4_script_writer`: generate HOI4 script with scope and syntax checks in mind.
- `hoi4_localisation_writer`: create localisation keys and text.
- `hoi4_gui_assistant`: work with GUI, GFX, and scripted GUI patterns.
- `hoi4_review`: review generated HOI4 mod content for load, scope, path, and localisation issues.

### Resources

- `resources/list` returns resource metadata.
- `resources/read` reads a selected URI.

Important URIs:

- `rhoiscribe://hoi4/latest-update`: bundled local snapshot of the latest recorded HOI4 update notes.
- `rhoiscribe://hoi4/knowledge/catalog`: JSON index of all bundled knowledge topics.
- `rhoiscribe://hoi4/knowledge/<topic_id>`: markdown rendering of one knowledge topic.

Example topic IDs include `script.triggers`, `script.effects`, `script.scopes`, `localisation.encoding`, `scripted_gui.dynamic_lists`, `gui.gfx_sprites`, `focus.basic_tree`, `events.country_event`, `decision.basic_category`, `history.states`, `map.adjacencies`, `technology.tech_trees`, `equipment.archetypes`, `ai.ai_strategy`, and `debug.common_errors`.

Each topic can include:

- `body`: concise guidance for agents.
- `syntax_blocks`: representative HOI4 script, GUI, GFX, localisation, or descriptor blocks.
- `relationships`: how the topic connects to other systems.
- `validation`: checks to run before generated content is trusted.
- `source_refs`: official wiki or reference pages used as source pointers.

### Tools

- `tools/list` returns available tools.
- `tools/call` runs one tool with JSON arguments.

Available tools:

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `validate_hoi4_paths`
- `format_paradox_script`

All generation tools support `dry_run`. When `dry_run` is `true`, no files are written and the response contains the planned files. When `dry_run` is `false`, `output_root` is required and must point to the target mod root, for example `<MOD_OUTPUT_ROOT>`.

Example `tools/call` arguments for `generate_localisation_batch`:

```json
{
  "language": "l_english",
  "file_stem": "my_mod_focuses",
  "key_prefix": "MYMOD",
  "entries": [
    {
      "id": "industrial_recovery",
      "title": "Industrial Recovery",
      "description": "Rebuild the industrial base."
    }
  ],
  "dry_run": true
}
```

The result contains a file plan such as:

```json
{
  "dry_run": true,
  "files": [
    {
      "path": "localisation/english/my_mod_focuses_l_english.yml",
      "encoding": "utf-8-bom",
      "summary": "HOI4 localisation file"
    }
  ],
  "messages": ["dry-run only; no files were written"]
}
```

For write mode:

```json
{
  "language": "l_english",
  "file_stem": "my_mod_focuses",
  "entries": [
    {
      "id": "industrial_recovery",
      "title": "Industrial Recovery"
    }
  ],
  "dry_run": false,
  "output_root": "<MOD_OUTPUT_ROOT>"
}
```

`validate_hoi4_paths` checks that generated paths are relative and stay inside supported HOI4 mod folders such as `common/`, `events/`, `history/`, `interface/`, `gfx/`, and `localisation/`.

`format_paradox_script` applies basic indentation to Paradox-style key/value script. It is a convenience formatter, not a full HOI4 semantic validator.

## Client Setup

See [docs/client-setup.md](docs/client-setup.md) for Roo Code, Codex, Claude Code, and generic MCP configuration examples.

## Knowledge Snapshot Updates

Runtime does not require network access. Bundled knowledge lives under [knowledge/hoi4](knowledge/hoi4). To refresh the latest HOI4 update snapshot, update [knowledge/hoi4/latest-update.md](knowledge/hoi4/latest-update.md) from official Paradox or Steam sources, then run verification and commit the change.

When expanding [knowledge/hoi4/catalog.json](knowledge/hoi4/catalog.json), keep every topic structured enough for agent use: category, file types, tags, syntax examples, relationships, validation guidance, and source references.

## Verification

Run the full local quality gate:

```powershell
.\scripts\verify.ps1
```

Equivalent commands:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

## Repository Rules

Agents must read [AGENTS.md](AGENTS.md) before development. Local implementation plans live in `plans/` and are intentionally ignored by git.
