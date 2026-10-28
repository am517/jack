# The Windups standard (draft 1, 2026-10-27)

A windup is a small program that does one job well. Windups call each other; a "super" windup
orchestrates others. This file is the contract. Anything shared lives in this repo.

## Roster

| Windup | Job | Status |
|---|---|---|
| Jack | fetches metadata (online databases) | building |
| Lauren | tags and embeds (writes metadata/art into files) | later |
| Sammy | the brains: picks the best value when sources disagree, summarizes | later |
| Daniel | database manager: populates and keeps everything (an archive-ophile) | later |
| Steve | finds local information in the database (search) | later |
| Sally | organizes files (rename, move, sort) | later |
| Tammy | super windup: ingests new media by orchestrating the others | later |
| Oliver | introducer/matchmaker between windups | much later |

## Rules

1. **One job per windup.** If a task has two jobs, it is two windups.
2. **Every windup is its own repo** (`am517/<name>`), Homebrew formula `windups-<name>`, MIT.
3. **One windup at a time.** Jack and this repo first; a new windup starts only when the current one is complete.
4. **Standalone.** Every windup works with no other windup installed, using a sane default when a
   helper can't be found (Jack without Sammy: longest summary wins; Tammy: "I'm sorry, I can't figure
   this out right now"). Helpers make results better, never required.
5. **All windups can call all windups.** A windup knows who to talk to without being told (see
   docs/communication.md). No user pipes (`a | b | c`) are needed or expected.
6. **The database is the source of truth.** All metadata enters `media.db`, then gets written to the
   file. **No sidecars**: a windup never reads `<file>.*` side files (an explicit `--download-only`
   export is the user's business; nothing reads it back).
7. **Efficiency above all**, especially in inter-windup communication.
8. **Never err on the side of simple; it's OK to err on the side of later.** When we build a windup we
   build it as well as we can. Design so later features (e.g. complex search) aren't blocked.
9. **Privacy: nothing leaves the machine unless the user opts in** (`allow_remote_metadata`).
   Fetching from public databases is a lookup, not sharing; sending our metadata to an API (e.g. a
   remote LLM) is sharing.
10. **Free services only**, where "free" includes a free tier (OMDb is fine).
11. **Secrets** never in git, logs, dry runs, errors or Debug output; masked when shown.

## Files and settings

| What | Where |
|---|---|
| Windup config | `~/.config/windups/<name>/config.toml` (`--config` overrides) |
| Shared settings | `~/.config/windups/windups.toml` (template: config/windups.toml) |
| Shared database | `~/.local/share/windups/media.db` (schema/) |
| Raw downloads (art, API responses) | `~/.local/share/windups/raw/`, pruned per `raw_max_age_days` |

- Every tweakable is a config value, never hard-coded. Config comments are short and for users.
- Precedence: flags > windup config > shared settings > built-in defaults.
- Every on/off flag has a `--no-X` form; `--set` saves flags into the config.

## Command line

- `--help`, `--version`, `--config <path>`, `--dry-run` (changes nothing, shows everything), `--set`.
- `--print` / `--print-format json` for humans and scripts (stdout = data, stderr = report).
- Exit codes: 0 all OK, 1 some items failed, 2 usage or config error.

## Quality bar

- Rust 2024, `cargo test` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Tests use fakes for the network; integration tests skip when external tools are missing.
- macOS first, Linux supported, Windows not a goal. File/folder names max 255 (characters on macOS, bytes on Linux).

## Design rules (Adam, 2026-10-28)
- **Treat each windup like a person.** When the right behavior is unclear, ask what a person would do. Example: with no database reachable, Jack writes a JSON next to the media (like jotting a note) — the one allowed exception to "no sidecars", and it is imported into media.db once available.
- **Modular.** One file does one thing (within reason); small focused modules.
