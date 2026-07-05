# HOI4 Latest Update Snapshot

Snapshot date: 2026-07-05

This local snapshot is based on official Steam and Paradox announcements plus the maintainer-provided local game version evidence visible on 2026-07-05. It is intentionally static so RHoiScribe can serve MCP resources without runtime network access.

## Latest Visible Update

- Current observed build: Operation Postern v1.19.2.0.a729
- Current observed checksum: d245
- Current observed date: 2026-07-05
- Major update: 1.19.0 Patch Notes
- Patch notes published: 2026-06-10
- Expansion release: Thunder at Our Gates
- Expansion release date: 2026-06-11
- Hotfix: 1.19.0.1
- Hotfix visible: 2026-06-12
- Source: official Steam News and Paradox announcements for Hearts of Iron IV, plus maintainer-provided local game version screenshot for the 1.19.2 build string

## Modding-Relevant Notes

- Treat 1.19.2 on the 1.19 branch as the current observed compatibility target for new release-critical work unless the user's installed `launcher-settings.json` reports another version.
- Use `supported_version="1.19.*"` only when the mod has actually been checked against the 1.19 branch.
- Thunder at Our Gates shipped alongside the 1.19 update, so agents should expect DLC-adjacent script, focus, country, and UI changes when comparing against older 1.18 assumptions.
- Operation Postern v1.19.2.0.a729 (d245) means error logs, validation runs, and game behavior should be checked against the user's installed patch before final release packaging.

## Agent Guidance

- Treat this as a local snapshot, not a live current-version guarantee.
- Prefer local syntax resources for generation.
- Read `launcher-settings.json` through `discover_hoi4_environment` when local version accuracy matters.
- When exact current patch compatibility matters, refresh this file from official Paradox or Steam sources before generating release-critical mod content.

## Source References

- https://steamcommunity.com/games/394360/announcements/detail/712277443918955865
- https://steamcommunity.com/app/394360/announcements
- https://forum.paradoxplaza.com/forum/forums/hearts-of-iron-iv.844/
- Maintainer-provided local game version screenshot: Operation Postern v1.19.2.0.a729 (d245), observed 2026-07-05
