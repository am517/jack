# Windups

Small programs that each do one job well and call each other. One Cargo workspace, one folder per windup.

## Origin

The windups come from Adam's view of the mind as a hive of many small selves, each with its own
identity and drive, negotiating influence and woven together into one.

A windup is a small program that does one job well. Windups call each other; a "super" windup
orchestrates others. This file is the contract. Anything shared lives in this repo.

## Cast list

- **Jack**: fetches and verifies metadata for a file (gathers facts), hands one entry to Daniel.
- **Daniel**: the database; takes handoffs, asks Larry and Ruth, stores, calls Walter.
- **Walter**: patrol; called by windups, schedules returns and calls windups back, then exits.
- **Larry**: hashing: exact BLAKE3 (partial first, full later) and the perceptual fingerprint.
- **Tom**: vectors/embeddings for meaning search.
- **Mary**: summaries (before Tom only).
- **Kate**: image conversion (JPEG to embed, JPEG XL to archive).
- **Sammy**: judgment, when evidence alone can't decide.
- **Julie**: verification against evidence; calls Sammy when judgment is needed.
- **Ruth**: builds the APA 7 work reference and work key.
- **Margaret**: directory; knows every windup's capabilities.
- **Oliver**: matchmaker; breaks requests down and teams windups up.
- **Ben**: rebuilds the windups for new platforms.
- **Rover**: refines other windups, bound by each one's purpose and success checks; proposes and tests, never applies unreviewed.

## Layout

- `windups/`: the shared crate and the contract (`windups/WINDUPS.md`, `windups/TODO.md`).
- One folder per windup, each with its `PROFILE.toml` (purpose and success checks), README and crate.

Build and test everything with `cargo build --workspace` and `cargo test --workspace`.

## License

MIT, see `LICENSE`.
