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
- [ ] Jack: `--capabilities`, registry and ledger once the communication design is approved
- [ ] Embeddings: sqlite-vec + local model (fastembed-rs) for texts

## Later, in order (not started)
1. **Lauren**: tags and embeds from the database (takes over Jack's built-in tagging; Jack keeps a fallback)
2. **Sammy**: chooses between sources (description, titles, dates, art) and summarizes; local model or opt-in remote API (`allow_remote_metadata`)
3. **Daniel**: database manager; populates and keeps everything
4. **Steve**: search the database (genres, people, dates, full text, meaning; graph later)
5. **Sally**: organizes files (takes over Jack's built-in sorting; Jack keeps a fallback)
6. **Tammy**: super windup; ingests new media by orchestrating the others
7. **Oliver**: introducer/matchmaker (much later)
8. **Margaret**: receptionist/directory of windups and capabilities (later, once there are too many to know personally)
- Update windup (files and database together), name TBD
