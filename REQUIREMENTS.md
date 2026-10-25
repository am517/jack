# Jack — movie & TV metadata tagger and sorter

Owner: Adam Moore. Builder: Coding Bot. Replaces iVI4 Pro (stopped working).
Pacing: built in small, independently useful phases over several weeks (token budget ~14%/day).

## Goal
Command-line tool: given a file or directory, look up movie/TV metadata from open online sources,
build the best composite metadata, write it into the files, and sort them into folders.

## Usage
    jack <file|dir> [flags]
    jack ~/Movies/some-movie.mp4 --imdb tt0473700
    jack ~/Movies/some-tv-series/ --thetvdb https://www.thetvdb.com/series/the-disastrous-life-of-saiki-k

## Flags
- --imdb <id|url>, --thetvdb <id|url>, --tmdb <id|url>, --youtube <url> (more providers later)
- -d, --download-only   fetch metadata/artwork, don't modify files
- -f, --from-file <path> apply previously downloaded metadata
- -o, --output <dir>    destination root for sorted files
- --dry-run             show what would change (recommended default for first runs)

## Config
- Main config (~/.config/jack/config.toml): API keys, provider priority per field, naming templates, output root.
- Per-file sidecar: `<filename>.config` beside the media file, same options as flags (alternative to flags).
- Precedence: flags > sidecar > main config. All tweakables live in config, not code (Adam's rule).

## Architecture
- Rust, modular: one module per provider behind a common trait; core (scan, match, merge), writer, sorter.
- Tag writing via exiftool / AtomicParsley (MP4/M4V, iTunes-style atoms like iVI4) or Rust crates where solid; MKV via mkvpropedit.
- Rebuild missing dependencies automatically.

## Providers
- Phase 1: TMDB (free key), TheTVDB v4 (key).
- Later: IMDb (official datasets, no scraping), OMDb (free key), Fanart.tv (art), YouTube (yt-dlp).
- Last/optional: Apple TV, Google Play, X (no public APIs; scraping is fragile).

## Sorting
- Movies: `<out>/Movies/Title (Year)/Title (Year).ext`
- TV: `<out>/TV/Show/Season 01/Show - S01E01 - Episode Title.ext`
- Templates configurable.

## Phases
1. Repo, CLI flags, config + sidecar loading, directory scan, dry-run.
2. Tag writer (exiftool/AtomicParsley), --download-only, --from-file.
3. TMDB + TheTVDB providers.
4. Filename matching, composite merge rules, sort/rename.
5. Extra providers, one at a time.

## Done when
Phases 1–4 tag and sort a real movie and a real TV season on Adam's Mac correctly, with dry-run first.
