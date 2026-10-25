# Homebrew (shared Swarm tap): draft

One tap, `homebrew-swarm`, holds formulas for all Swarm apps (jack, saikanji, ...).
Nothing is published yet.

1. Push jack to GitHub, then tag a release: `git tag v0.1.0 && git push --tags`.
2. Get the tarball checksum:
   `curl -L https://github.com/OWNER/jack/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256`
3. In `jack.rb`, fill in `homepage`, `url`, `sha256`, `license` (all marked TODO).
4. Create the tap repo `OWNER/homebrew-swarm` (or reuse the existing one) and
   copy `jack.rb` to `Formula/jack.rb` there, beside `Formula/saikanji.rb`.
5. Test locally: `brew install --build-from-source OWNER/swarm/jack && brew test jack && brew audit --strict jack`.
6. Install: `brew tap OWNER/swarm && brew install jack`.
