# RHoiScribe

[English](../README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

RHoiScribe - локальный MCP-сервер для AI agents, которые работают с модами Hearts of Iron IV. Он дает Codex, Claude Code и другим MCP-совместимым клиентам локальный справочный слой по HOI4 modding и инструменты для генерации файлов, читаемых игрой.

Цель проекта проста: уменьшить лишнюю работу agents из-за повторного веб-поиска, устаревших предположений, небезопасных путей, пропущенной кодировки локализации и Paradox script, который выглядит правдоподобно, но не загружается в игре.

## Для кого

- Для авторов модов, которые хотят давать AI agents более сильный локальный контекст.
- Для agent workflows, где prompts, resources и tools должны быть доступны через один MCP server.
- Для офлайн-сессий или разработки с минимальным поиском, где agent должен читать встроенные HOI4 guidance перед записью файлов.
- Для команд, которым нужны предсказуемые mod-root пути и проверяемая форма сгенерированного вывода.

## Что получают agents

### Prompts

Встроенные prompts помогают с:

- планированием mod feature
- написанием HOI4 script
- написанием localisation
- работой с GUI, GFX и scripted GUI
- проверкой сгенерированного контента

Текущие имена prompts: `hoi4_mod_planner`, `hoi4_script_writer`, `hoi4_localisation_writer`, `hoi4_gui_assistant`, `hoi4_review`.

### Resources

Agents могут читать локальные resources вместо старта с пустого prompt:

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

Knowledge catalog структурирован для agents. Topics содержат category, file types, tags, syntax examples, relationships с другими системами HOI4, validation guidance и source references. Текущее покрытие включает script basics, scopes, triggers, effects, modifiers, variables, arrays, localisation, scripted localisation, scripted triggers/effects, GUI, scripted GUI, focuses, events, decisions, ideas, characters, history, map files, technology, equipment, units, AI, diplomacy, game rules, defines, bookmarks, audio и common loading errors.

### Tools

Agents могут вызывать tools для повторяемой генерации и проверки:

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `validate_hoi4_paths`
- `format_paradox_script`

Generation tools поддерживают dry-run preview. В write mode требуется `output_root`, а запись идет только по путям относительно корня целевого мода.

## Быстрый старт

Соберите сервер:

```powershell
cargo build --release
```

Укажите release binary в MCP-клиенте:

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

Для Linux и macOS:

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe
```

Запускайте напрямую только если хотите вручную стартовать stdio MCP server:

```powershell
.\target\release\rhoiscribe.exe
```

Примеры конфигурации для Codex, Claude Code и generic MCP см. в [client-setup.md](client-setup.md).

## MCP Surface

После запуска RHoiScribe клиентом agent может использовать стандартные MCP methods:

- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- `tools/list`
- `tools/call`

Пример чтения resource:

```text
rhoiscribe://hoi4/knowledge/scripted_gui.dynamic_lists
```

Пример аргументов `tools/call` для dry-run локализации:

```json
{
  "language": "l_simp_chinese",
  "file_stem": "common/autonomy/CHI",
  "key_prefix": "CHI",
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

Write mode добавляет корень вывода мода:

```json
{
  "language": "l_simp_chinese",
  "file_stem": "common/autonomy/CHI",
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

В write mode файл localisation записывается с UTF-8 BOM.
Если мод пользователя уже использует вложенные папки localisation, можно передать `file_stem` вроде `common/autonomy/CHI` или полный mod-relative path вроде `localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml`.

## Модель вывода

Generation tools возвращают структурированный план файлов:

```json
{
  "dry_run": true,
  "files": [
    {
      "path": "localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml",
      "encoding": "utf-8-bom",
      "summary": "HOI4 localisation file"
    }
  ],
  "messages": ["dry-run only; no files were written"]
}
```

Пути являются относительными к моду и могут использовать вложенные HOI4-readable папки, если это соответствует workspace пользователя. Небезопасные пути, пути с drive prefix и directory traversal отклоняются до записи.
