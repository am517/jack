-- Schema v2 (2026-10-28): unresolved status, source names + local display names,
-- conflicts between sources, embeddings, windup profiles.

-- Why something is unresolved. kind 'time': retry after settings.unresolved_retry_days
-- (a future tidy-up windup does that). kind 'resource': wait until needs_windup exists.
CREATE TABLE unresolved_reasons (
  reason        TEXT PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('time', 'resource')),
  needs_windup  TEXT,                          -- resource reasons: who can settle it
  description   TEXT
);
INSERT INTO unresolved_reasons VALUES
  ('no_match',         'time',     NULL,    'no source knows it (yet)'),
  ('ambiguous',        'time',     NULL,    'more than one exact match; not possible with the available information'),
  ('source_error',     'time',     NULL,    'a source failed (network, quota)'),
  ('conflict',         'resource', 'sammy', 'sources disagree; needs a judgement'),
  ('needs_llm_windup', 'resource', 'sammy', 'needs reading/judgement beyond exact rules');

-- One row per unresolved thing (media, or a file Jack could not attach to media yet).
CREATE TABLE unresolved (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER REFERENCES media(id) ON DELETE CASCADE,
  file_path     TEXT,
  windup        TEXT NOT NULL,                 -- who gave up (jack)
  reason        TEXT NOT NULL REFERENCES unresolved_reasons(reason),
  detail        TEXT,                          -- e.g. the candidate titles
  status        TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_checked  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  resolved_by   TEXT,
  CHECK (media_id IS NOT NULL OR file_path IS NOT NULL)
);
CREATE INDEX unresolved_open ON unresolved(status, reason, last_checked);
CREATE INDEX unresolved_media ON unresolved(media_id);

-- Local display names: auto-filled from a source name, user-editable; source names stay untouched.
ALTER TABLE media    ADD COLUMN display_name TEXT;
ALTER TABLE media    ADD COLUMN display_name_set_by TEXT;  -- 'auto:<source>' | 'user'
ALTER TABLE entities ADD COLUMN display_name TEXT;
ALTER TABLE entities ADD COLUMN display_name_set_by TEXT;

-- Every name any source uses for a media row or entity (channel title, TMDB name, filename...).
CREATE TABLE source_names (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER REFERENCES media(id) ON DELETE CASCADE,
  entity_id     INTEGER REFERENCES entities(id) ON DELETE CASCADE,
  source        TEXT NOT NULL,                 -- youtube | tmdb | embedded | filename | ...
  name          TEXT NOT NULL,
  fetched_at    TEXT DEFAULT CURRENT_TIMESTAMP,
  CHECK ((media_id IS NULL) != (entity_id IS NULL))
);
-- (NULLs are distinct in UNIQUE, so one index per owner)
CREATE UNIQUE INDEX source_names_media ON source_names(media_id, source, name) WHERE media_id IS NOT NULL;
CREATE UNIQUE INDEX source_names_entity ON source_names(entity_id, source, name) WHERE entity_id IS NOT NULL;
CREATE INDEX source_names_name ON source_names(name COLLATE NOCASE);

-- Sources disagreeing on a field. Recorded, never settled by assumption.
CREATE TABLE conflicts (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  field         TEXT NOT NULL,
  values_json   TEXT NOT NULL,                 -- [{"source": "...", "value": "..."}, ...]
  status        TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
  chosen_value  TEXT,
  resolved_by   TEXT,
  detected_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (media_id, field)
);

-- Vectors for meaning search. Plain little-endian f32 BLOBs, so any small local model fits
-- (swappable: model is part of the key). sqlite-vec can index these later.
CREATE TABLE embeddings (
  id            INTEGER PRIMARY KEY,
  owner         TEXT NOT NULL,                 -- text | media | entity | windup
  owner_id      TEXT NOT NULL,                 -- row id, or windup name
  model         TEXT NOT NULL,
  dim           INTEGER NOT NULL,
  vector        BLOB NOT NULL,
  content_hash  TEXT,                          -- re-embed when the source text changes
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (owner, owner_id, model)
);

-- Windup profiles (copied from each repo's PROFILE.toml, the source of truth) for Margaret.
CREATE TABLE windups (
  name          TEXT PRIMARY KEY,
  version       TEXT,
  profile       TEXT NOT NULL,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
