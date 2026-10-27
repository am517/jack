# Homebrew

`windups-jack.rb` (installs the `jack` command) and `formula_renames.json` (old name `jack` still works) mirror `Formula/windups-jack.rb` in [am517/homebrew-tap](https://github.com/am517/homebrew-tap).

```zsh
brew install am517/tap/windups-jack
```

## Embedded API keys (optional)

`jack` reads `JACK_TMDB_TOKEN`, `JACK_TVDB_KEY` and `JACK_OMDB_KEY` at compile time. The formula forwards
`HOMEBREW_JACK_TMDB_TOKEN` / `HOMEBREW_JACK_TVDB_KEY` / `HOMEBREW_JACK_OMDB_KEY` if they're set:
`HOMEBREW_JACK_TMDB_TOKEN=... HOMEBREW_JACK_TVDB_KEY=... HOMEBREW_JACK_OMDB_KEY=... brew install --build-from-source am517/tap/windups-jack`.
By default no keys are embedded, so put them in `~/.config/windups/jack/config.toml`.
Keys embedded in a binary can be extracted, so never ship private keys in a public bottle.

## Releasing a new version

1. Bump `version` in `Cargo.toml`, commit, then `git tag -a vX.Y.Z && git push origin vX.Y.Z`.
2. `curl -sL https://github.com/am517/jack/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256`
3. Update `url` and `sha256` in the tap's `Formula/windups-jack.rb` and copy it here.
4. If brew is available: `brew audit --strict am517/tap/windups-jack && brew test windups-jack`.
