# Jack

Rust CLI that tags and sorts movie and TV files (replaces iVI4 Pro). First app in the Windups group of small named apps.

- Requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Config: `config/jack.toml` (keep tweakable values here, not hard-coded)

Status: phases 1-4 done: CLI, config + sidecar, scan, tag writing, TMDB/TheTVDB lookup, filename matching, composite merge, sort/rename.

## Install

```zsh
brew install am517/tap/jack
mkdir -p ~/.config/windups/jack && cp "$(brew --prefix)/share/jack/config.toml.example" ~/.config/windups/jack/config.toml
```

## Build from source (macOS, zsh)

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
jack "Dune (1984).mkv" --name "Dune" --year 1984  # override the filename guess
jack "Dune (1984).mkv" --art landscape             # backdrop / episode still instead of poster
jack --set --art landscape --providers tmdb,omdb      # save flags as defaults in the main config (no paths: save only)
jack ~/TV/saiki-k/ --thetvdb https://www.thetvdb.com/series/the-disastrous-life-of-saiki-k -o ~/Media
jack ~/Movies/Inbox -d                             # download metadata + poster only (<file>.meta.json, <file>.poster.jpg)
jack ~/Movies/Inbox -f ~/Movies/Inbox -o ~/Media   # apply saved metadata, then sort
```

## Config

- Main config: `~/.config/windups/jack/config.toml` or `--config <path>`; deep-merged over the built-in defaults in `config/jack.toml`, which documents every key.
- Per-file sidecar: `<file>.config` (e.g. `movie.mp4.config`) with the same keys as the flags (`imdb`, `tmdb`, `thetvdb`, `output`, `dry_run`, ...).
- Precedence: flags > sidecar > main config > built-in defaults. `~` is expanded in paths.
- Tweakable there: extensions, tag tool and paths, provider order, keys and base URLs, per-field merge priority, filename regexes, sort templates, move/copy.

Packaging: see `packaging/homebrew/` (mirror of the `am517/tap` formula).

Database ids (IMDb/TMDB/TheTVDB, plus series/episode ids for TV) are written into the file
(MP4 iTunes freeform atoms, MKV global tags) and read back on later runs, so re-tagging skips
the filename guess. Tag names are under `[tagger.mp4_id_tags]` / `[tagger.mkv_id_tags]`.

OMDb (optional, needs a key) adds ratings (IMDb, Rotten Tomatoes, Metacritic), rated and awards,
written with the same tag maps (MP4 freeform atoms, e.g. `IMDB_RATING`; MKV simple tags, e.g.
`LAW_RATING`), plus the iTunes content rating (iTunEXTC) for `rated`. Without a key it is skipped
with a warning.

## API keys

Keys come from the config file first, then from project keys embedded at build time:

```zsh
JACK_TMDB_TOKEN=... JACK_TVDB_KEY=... JACK_OMDB_KEY=... cargo build --release
```

Config `[providers.tmdb] read_token`/`api_key` `[providers.thetvdb] api_key` and `[providers.omdb] api_key` override embedded keys. The TheTVDB subscriber `pin` is config-only. With neither, Jack reports which key to set. Never commit keys (`.env`, `*.key` and `secrets*` are git-ignored). Keys embedded in a binary can be extracted from it.

## Attribution

- Metadata provided by TheTVDB: https://thetvdb.com
- This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org

- Ratings, rated and awards from OMDb (The Open Movie Database): https://www.omdbapi.com (courtesy credit; OMDb's terms require none, but limit use to personal, non-commercial purposes)

These lines are also shown in `jack --help`.

## License

MIT, see [LICENSE](LICENSE).
