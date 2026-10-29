-- v7: a work gets one permanent local id (works.id, never reused or changed). Its reference and key
-- may improve (a provisional reference becomes the original-language one); per-service ids
-- (imdb, tmdb, tvdb, youtube, ...) live beside it in work_ids.
CREATE TABLE works_v7 (
  id          INTEGER PRIMARY KEY,
  key         TEXT NOT NULL UNIQUE,          -- wk1:<blake3 of the normalized reference>
  reference   TEXT NOT NULL,
  provisional INTEGER NOT NULL DEFAULT 1,    -- 1: not yet from original title/language/release
  first_seen  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_seen   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO works_v7(key, reference, first_seen, last_seen) SELECT key, reference, first_seen, last_seen FROM works;
CREATE TABLE work_ids (
  work_id INTEGER NOT NULL REFERENCES works_v7(id),
  source  TEXT NOT NULL,                     -- imdb_id, tmdb_id, thetvdb_id, youtube_video_id, ...
  value   TEXT NOT NULL,
  PRIMARY KEY (source, value)
);
INSERT OR IGNORE INTO work_ids(work_id, source, value)
  SELECT w.id, j.key, j.value FROM works o JOIN works_v7 w ON w.key = o.key, json_each(coalesce(o.ids, '{}')) j;
ALTER TABLE media ADD COLUMN work_id INTEGER REFERENCES works_v7(id);
UPDATE media SET work_id = (SELECT id FROM works_v7 WHERE key = media.work_key);
DROP INDEX IF EXISTS media_work;
ALTER TABLE media DROP COLUMN work_key;
DROP TABLE works;
ALTER TABLE works_v7 RENAME TO works;
CREATE INDEX media_work ON media(work_id);

-- When an entry was last looked at (Walter decides from this, the stage and the reason).
ALTER TABLE media ADD COLUMN last_checked TEXT;

-- Capabilities Walter has seen (windups, sources). A new one wakes what waits for it.
CREATE TABLE capabilities (
  name       TEXT PRIMARY KEY,
  kind       TEXT NOT NULL,                  -- windup | source
  first_seen TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- The entry's last_checked when its return was scheduled: a newer check means reschedule.
ALTER TABLE returns ADD COLUMN checked_at TEXT;
