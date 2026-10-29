-- v3: a reason only the user can settle: a flag such as --year or --imdb picks the one meant.
INSERT OR IGNORE INTO unresolved_reasons VALUES
  ('needs_flags', 'resource', 'user', 'needs a flag from the user (--year, --imdb, --name); the information available cannot tell');

-- Decisions made by escalation (Sammy, or a person), with provenance: who, at which step, when.
CREATE TABLE decisions (
  id          INTEGER PRIMARY KEY,
  media_id    INTEGER REFERENCES media(id) ON DELETE CASCADE,
  file_path   TEXT,
  windup      TEXT NOT NULL,                 -- the windup whose step it was (jack)
  step        TEXT NOT NULL,                 -- parse | lookup | match | refine | conflict | ...
  source      TEXT NOT NULL,                 -- who decided: sammy | user
  decision    TEXT NOT NULL,                 -- JSON: the values decided
  note        TEXT,                          -- why, in the decider's words
  decided_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX decisions_media ON decisions(media_id);
