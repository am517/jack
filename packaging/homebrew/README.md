# Homebrew (shared Windups tap): draft

One tap, `homebrew-tap`, holds formulas for all Windups apps (jack, saikanji, ...).
Nothing is published yet.

## Embedded API keys (optional)

`jack` reads `JACK_TMDB_TOKEN` and `JACK_TVDB_KEY` at compile time. In the formula's
`install`, set them from the builder's environment before `cargo install`:

```ruby
ENV["JACK_TMDB_TOKEN"] = ENV.fetch("HOMEBREW_JACK_TMDB_TOKEN", "")
ENV["JACK_TVDB_KEY"]   = ENV.fetch("HOMEBREW_JACK_TVDB_KEY", "")
```

Build with `HOMEBREW_JACK_TMDB_TOKEN=... HOMEBREW_JACK_TVDB_KEY=... brew install --build-from-source ...`
(Homebrew passes `HOMEBREW_*` variables through), or from a CI secret when building bottles.
Caveat: keys embedded in a public bottle or binary can be extracted (e.g. with `strings`); use
keys you're allowed to distribute, or leave them out and have users set them in
`~/.config/windups/jack/config.toml`.

## Publish

1. Push jack to GitHub, then tag a release: `git tag v0.1.0 && git push --tags`.
2. Get the tarball checksum:
   `curl -L https://github.com/OWNER/jack/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256`
3. In `jack.rb`, fill in `homepage`, `url`, `sha256`, `license` (all marked TODO).
4. Create the tap repo `OWNER/homebrew-tap` (or reuse the existing one) and
   copy `jack.rb` to `Formula/jack.rb` there, beside `Formula/saikanji.rb`.
5. Test locally: `brew install --build-from-source OWNER/windups/jack && brew test jack && brew audit --strict jack`.
6. Install: `brew tap OWNER/windups && brew install jack`.
