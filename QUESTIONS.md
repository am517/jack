# Open questions for Adam

## Schema (schema/)
1. Schema v1 OK as drafted (entities with roles, candidates as real rows, sub-genres, relations)?
2. Raw API responses: keep them in `raw_json` on each row (as drafted), or as files under `raw_dir`?
3. Recompressing kept images to AVIF/JXL/HEIC: fine for the raw archive. MP4 cover art (`covr`) only
   allows JPEG/PNG (and BMP), so embedded art stays JPEG/PNG. OK?
4. Embedding model: bge-small / all-MiniLM (384 dims, ~30-130 MB download on first use) OK?

## Who writes the database
5. Until Daniel exists, Jack writes media.db itself (rule 4: standalone), via the shared crate. OK?
6. Who owns migrations: the shared crate in this repo (recommended), or Daniel later?

## Communication (docs/communication.md)
7. Direct calls + a task ledger in media.db (recommended), or a broker/queue from the start?
8. Discovery by `windups-*` on PATH + `--capabilities`: OK?

## Order
9. Next windup after Jack: Lauren (as in TODO.md) or Daniel/Sammy first?

## Resolved by the person rule (2026-10-28)
- 5: Yes — a person who needs something written down writes it themselves. Jack writes media.db via the shared crate; with no DB, a JSON next to the media.
- 6: The shared crate owns migrations (one rulebook everyone reads). Daniel can call it later.
- 2: Raw responses in `raw_json` on each row (a person keeps the receipt with the item); large binaries (art) as files under raw_dir.
