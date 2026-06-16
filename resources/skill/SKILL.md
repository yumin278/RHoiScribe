---
name: rhoiscribe-hoi4
description: Use when an agent needs local Hearts of Iron IV modding prompts, resources, or tools from a downloaded RHoiScribe Skill package without configuring an MCP server.
---

# RHoiScribe HOI4

Use the RHoiScribe executable in the same directory as this `SKILL.md` when HOI4 modding work needs local prompts, bundled resources, or batch tools.

## Find The Binary

Use the executable shipped beside this file:

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

On Linux or macOS, run `chmod +x ./rhoiscribe-linux-x86_64` or `chmod +x ./rhoiscribe-macos-universal` if the shell reports a permission error.

## Direct Commands

These commands return JSON and use the same prompt, resource, and tool catalogs as the MCP server:

```bash
./rhoiscribe-linux-x86_64 --skill list-tools
./rhoiscribe-linux-x86_64 --skill list-resources
./rhoiscribe-linux-x86_64 --skill list-prompts
./rhoiscribe-linux-x86_64 --skill read-resource "rhoiscribe://hoi4/latest-update"
./rhoiscribe-linux-x86_64 --skill get-prompt "hoi4_mod_planner" '{"request":"plan an industrial focus branch"}'
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

Use the platform executable name for the current system. On Windows, quote JSON for PowerShell:

```powershell
.\rhoiscribe-windows-x86_64.exe --skill call-tool "format_paradox_script" '{ "script": "focus={id=TAG_focus cost=10}" }'
```

## Agent Workflow

Use this Skill as a launcher for the executable-backed RHoiScribe catalog:

- Run `--skill list-prompts`, then `--skill get-prompt` for the task prompt before planning or editing.
- Run `--skill list-resources`, then `--skill read-resource` for relevant HOI4 knowledge topics before relying on memory or web search.
- Run `--skill list-tools` before tool use and follow each returned tool description and JSON input schema.
- Use `--skill call-tool` for the same tools that the MCP server exposes; tool outputs are JSON and should drive the next step.
- After RHoiScribe has been used for file-changing HOI4 work, get the current prompt/resource guidance again if the workflow is unclear instead of treating this `SKILL.md` as the full rulebook.
