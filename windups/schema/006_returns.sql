-- Every entry that isn't complete has a scheduled return: a time, a resource it waits for, or both.
CREATE TABLE returns (
  media_id      INTEGER PRIMARY KEY REFERENCES media(id),
  stage         TEXT NOT NULL,
  reason        TEXT NOT NULL DEFAULT '',
  next_retry_at TEXT,                      -- UTC 'YYYY-MM-DD HH:MM:SS'; due when <= now
  waiting_for   TEXT,                      -- a capability (e.g. 'sammy', 'new_source'); due when it appears
  attempts      INTEGER NOT NULL DEFAULT 0,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CHECK (next_retry_at IS NOT NULL OR waiting_for IS NOT NULL)
);
CREATE INDEX returns_due ON returns(next_retry_at);
