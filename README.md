# Jack

Rust CLI that tags and sorts movie and TV files (replaces iVI4 Pro). First app in the Windups group of small named apps.

- Requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Config: `config/jack.toml` (keep tweakable values here, not hard-coded)

Status: phases 1-4 done: CLI, config + sidecar, scan, tag writing, TMDB/TheTVDB lookup, filename matching, composite merge, sort/rename.

## Setup (macOS, zsh)

```zsh
brew install atomicparsley exiftool mkvtoolnix
cargo build --release
mkdir -p ~/.config/windups/jack && cp config/jack.toml ~/.config/windups/jack/config.toml
# then add your keys under [providers.tmdb] and [providers.thetvdb]
```

## Usage

```zsh
jack ~/Movies/Inbox --dry-run -o ~/Media          # preview tags + renames; changes nothing
jack ~/Movies/the.matrix.1999.mp4 --imdb tt0133093 # tag in place
jack ~/TV/saiki-k/ --thetvdb https://www.thetvdb.com/series/the-disastrous-life-of-saiki-k -o ~/Media
jack ~/Movies/Inbox -d                             # download metadata + poster only (<file>.meta.json, <file>.poster.jpg)
jack ~/Movies/Inbox -f ~/Movies/Inbox -o ~/Media   # apply saved metadata, then sort
```

## Config

- Main config: `~/.config/windups/jack/config.toml` or `--config <path>`; deep-merged over the built-in defaults in `config/jack.toml`, which documents every key.
- Per-file sidecar: `<file>.config` (e.g. `movie.mp4.config`) with the same keys as the flags (`imdb`, `tmdb`, `thetvdb`, `output`, `dry_run`, ...).
- Precedence: flags > sidecar > main config > built-in defaults. `~` is expanded in paths.
- Tweakable there: extensions, tag tool and paths, provider order, keys and base URLs, per-field merge priority, filename regexes, sort templates, move/copy.

Packaging: see `packaging/homebrew/` (draft formula for the shared Windups tap).

## API keys

Keys come from the config file first, then from project keys embedded at build time:

```zsh
JACK_TMDB_TOKEN=... JACK_TVDB_KEY=... cargo build --release
```

Config `[providers.tmdb] read_token`/`api_key` and `[providers.thetvdb] api_key` override embedded keys. The TheTVDB subscriber `pin` is config-only. With neither, Jack reports which key to set. Never commit keys (`.env`, `*.key` and `secrets*` are git-ignored). Keys embedded in a binary can be extracted from it.

## Attribution

- Metadata provided by TheTVDB: https://thetvdb.com
- This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org

Both lines are also shown in `jack --help`.
