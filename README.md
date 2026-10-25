# Jack

Rust CLI that tags and sorts movie and TV files (replaces iVI4 Pro). First app in the Swarm group of small named apps.

- Requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Config: `config/jack.toml` (keep tweakable values here, not hard-coded)

Status: phase 1 done (CLI flags, config + sidecar loading, scan, dry-run). No tagging/providers yet.

```zsh
cargo build --release
./target/release/jack ~/Movies --dry-run
```

Config: `~/.config/jack/config.toml` or `--config <path>`; per-file sidecar `<file>.config` (e.g. `movie.mp4.config`). Precedence: flags > sidecar > main config > built-in defaults (`config/jack.toml`). Media extensions are set in config.

