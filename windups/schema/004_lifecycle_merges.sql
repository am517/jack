-- v4: identification is a process, not a goal. Every entry (a chosen media row) walks stages over
-- time; each move is kept in status_history. Stages and allowed moves are rows, so new ones can be
-- added without code changes. Entries are never deleted: they are merged, traceably.
CREATE TABLE lifecycle_states (
  state       TEXT PRIMARY KEY,
  ord         INTEGER NOT NULL,              -- rough order along the way (for display)
  description TEXT NOT NULL
);
INSERT INTO lifecycle_states VALUES
  ('undetermined',   0, 'file purgatory: only the file''s and the user''s own values; nothing looked up yet, or lookup failed'),
  ('not_identified', 1, 'looked up, but no source answer could be settled on (see unresolved for why)'),
  ('unverified',     2, 'matched by strong hints (name, year, user-given name/year or channel mapping), not by an id'),
  ('incomplete',     3, 'identified by an id; something is still open (a conflict, a missing field)'),
  ('complete',       4, 'identified by an id; nothing open'),
  ('merged',         9, 'folded into another entry (see merged_into and merges); kept, never reused');

CREATE TABLE lifecycle_transitions (
  from_state  TEXT NOT NULL REFERENCES lifecycle_states(state),
  to_state    TEXT NOT NULL REFERENCES lifecycle_states(state),
  PRIMARY KEY (from_state, to_state)
);
-- any working stage may move to any other (an id can turn out wrong; new facts can reopen), or be merged;
-- merged is final
INSERT INTO lifecycle_transitions
  SELECT a.state, b.state FROM lifecycle_states a, lifecycle_states b
  WHERE a.state <> 'merged' AND a.state <> b.state;

ALTER TABLE media ADD COLUMN lifecycle TEXT REFERENCES lifecycle_states(state);  -- NULL for candidate rows
ALTER TABLE media ADD COLUMN merged_into INTEGER REFERENCES media(id);
CREATE INDEX media_lifecycle ON media(lifecycle);

CREATE TABLE status_history (
  id          INTEGER PRIMARY KEY,
  media_id    INTEGER NOT NULL REFERENCES media(id),
  from_state  TEXT REFERENCES lifecycle_states(state),   -- NULL: first stage
  to_state    TEXT NOT NULL REFERENCES lifecycle_states(state),
  actor       TEXT NOT NULL,                             -- windup or person: jack | sammy | user
  reason      TEXT,
  at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX status_history_media ON status_history(media_id, id);

CREATE TABLE merges (
  id          INTEGER PRIMARY KEY,
  from_id     INTEGER NOT NULL REFERENCES media(id),
  into_id     INTEGER NOT NULL REFERENCES media(id),
  actor       TEXT NOT NULL,
  reason      TEXT,
  at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- entries are never deleted (candidate rows, lifecycle NULL, may be refreshed)
CREATE TRIGGER media_keep_entries BEFORE DELETE ON media WHEN old.lifecycle IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'entries are never deleted; merge them instead'); END;

-- existing entries onto the lifecycle: open unresolved -> not_identified; otherwise unverified
-- (no record of how they were matched, so not assumed id-verified)
UPDATE media SET lifecycle = CASE
    WHEN EXISTS (SELECT 1 FROM unresolved u WHERE u.media_id = media.id AND u.status = 'open') THEN 'not_identified'
    ELSE 'unverified' END
  WHERE status = 'chosen';
INSERT INTO status_history(media_id, from_state, to_state, actor, reason)
  SELECT id, NULL, lifecycle, 'migration', 'schema v4' FROM media WHERE lifecycle IS NOT NULL;
