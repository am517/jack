# Timeline / to-do

One windup at a time; a new one starts only when the current one is complete (Adam, 2026-10-27).

## Now: Jack + this repo, until both are satisfactory
- [x] WINDUPS.md standard (draft 1), shared settings, schema v1 draft with example searches
- [x] Jack: no sidecars, filename_max 255 (chars on macOS, bytes on Linux), longest description fallback
- [x] Adam reviewed schema v1 and communication proposal (2026-10-28)
- [x] Shared `windups` crate: settings, paths, open/migrate media.db (user_version), raw pruning
- [ ] Jack writes everything it fetches into media.db (candidates, all art, texts, entities, genres), then tags from it
- [ ] Jack: new free sources in order: Wikidata (groupings, cross-ids), TVmaze, iTunes art, YouTube trailers; Wikipedia text as a description fallback
- [ ] Jack: cast/crew and studios into entities
- [x] Jack: `--capabilities` (prints PROFILE.toml)
- [ ] Jack: registry entry and task ledger in media.db
- [x] Schema v2: unresolved (reason, time/resource, last_checked), source_names + display names, conflicts, embeddings, windups profiles
- [x] Vectors: f32 BLOB storage + brute-force cosine search + `Embedder` trait (shared crate, no heavy deps)
- [ ] Embeddings: plug in a small local model (fastembed-rs) behind `Embedder`; embed texts, media, windup profiles; sqlite-vec index when size needs it
- [ ] Tidy-up windup (name TBD): retry time-based unresolved items after `unresolved_retry_days`
- [ ] Review what a windup did to a given file (e.g. a macOS QuickLook metadata plugin, or similar). Implementation TBD
- [ ] Sammy: implement Jack's `Escalation` (per-step decide-or-defer, recorded in `decisions`) and `Explainer`
- [ ] Post-run review pass (Jack `[review]`, off by default): Sammy implements `Reviewer`; per field in chunks with minimal context; flags only, never rewrites; record flags in `decisions` (step=review) once media ids are at hand
- [ ] Wikidata and YouTube searches still take one result (srlimit=1 / maxResults=1); keep the full list like the others
- [ ] Store each windup's PROFILE.toml in the `windups` table (and embed it) on run

## Later, in order (not started)
1. **Lauren**: tags and embeds from the database (takes over Jack's built-in tagging; Jack keeps a fallback)
2. **Sammy**: chooses between sources (description, titles, dates, art) and summarizes; local model or opt-in remote API (`allow_remote_metadata`)
3. **Daniel**: database manager; populates and keeps everything
4. **Steve**: search the database (genres, people, dates, full text, meaning; graph later)
5. **Sally**: organizes files (takes over Jack's built-in sorting; Jack keeps a fallback)
6. **Tammy**: super windup; ingests new media by orchestrating the others
7. **Oliver**: manager/matchmaker (much later; revisit after Margaret)
8. **Margaret**: receptionist/directory of windups and capabilities (later, once there are too many to know personally)
- Update windup (files and database together), name TBD

- Real perceptual fingerprinter (jack `fingerprint.rs`): sampled-frame pHashes via ffmpeg (robust to
  re-encode/resize/crop/rotation/aspect), optional Chromaprint audio; tune `max_distance`. Then a way for
  the user/Sammy to accept/reject `proposals` (accept → `lifecycle::merge`).
- Returns: Walter patrols when another windup calls him (no timers, by Adam's rule). Who calls Walter, and how often, is open (Tammy after each ingest?).
- Move Jack's `store.rs` into a real Daniel crate; Jack keeps only the handoff.
- Larry: a full-hash pass over partial-hashed files as resources permit.
- Larry: real perceptual fingerprint, aligning by audio first, then frames.
- Tom, Mary, Kate: real implementations (Kate converts Jack's art).
- Walter's name pending Adam's OK.
