-- v5: two ways to recognize things again.
-- files.hash (already there) re-finds a renamed or moved file, undetermined entries included.
-- works group versions of the same work (encodes, releases, cuts): key = hash of a normalized
-- APA 7 style reference; ids known for the work are kept beside it.
CREATE TABLE works (
  key         TEXT PRIMARY KEY,              -- wk1:<blake3 of the normalized reference>
  reference   TEXT NOT NULL,                 -- e.g. "The Matrix. (1999). [Film]."
  ids         TEXT,                          -- JSON: work-level ids seen (imdb_id, tmdb_id, ...)
  first_seen  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_seen   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE media ADD COLUMN work_key TEXT REFERENCES works(key);
CREATE INDEX media_work ON media(work_key);

-- Perceptual fingerprint (survives re-encode, resize, crop): a strong hint only, never proof.
ALTER TABLE files ADD COLUMN fingerprint TEXT;
-- Suggestions waiting for the user or Sammy; never acted on automatically.
CREATE TABLE proposals (
  id          INTEGER PRIMARY KEY,
  kind        TEXT NOT NULL,                 -- possible_same
  media_id    INTEGER NOT NULL REFERENCES media(id),
  other_id    INTEGER NOT NULL REFERENCES media(id),
  score       REAL,                          -- distance or similarity, per `source`
  source      TEXT NOT NULL,                 -- who proposed (fingerprinter name)
  status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'accepted', 'rejected')),
  decided_by  TEXT,
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (kind, media_id, other_id)
);
