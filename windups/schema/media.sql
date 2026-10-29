-- Windups shared database, schema v1 (draft). SQLite 3.35+, FTS5.
-- Location: ~/.local/share/windups/media.db. Vectors: vectors.sql (optional, sqlite-vec).
-- Everything downloaded is kept as real rows: alternatives/wrong results are status 'candidate'
-- (or 'rejected') and point at the chosen row via canonical_id. No separate fetches table.
-- Many-to-many links use junction tables, so every link works both ways (media <-> genre, ...).
PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;

-- Movies, shows, seasons, episodes (and any later video kind).
CREATE TABLE media (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL,                 -- movie | show | season | episode | ...
  parent_id     INTEGER REFERENCES media(id),  -- episode -> season -> show
  title         TEXT,
  original_title TEXT,
  sort_title    TEXT,
  language      TEXT,                          -- original language
  year          INTEGER,
  date          TEXT,                          -- release / air date, ISO 8601
  season        INTEGER,
  episode       INTEGER,
  runtime_min   INTEGER,
  rated         TEXT,                          -- content rating (PG-13, TV-MA, ...)
  status        TEXT NOT NULL DEFAULT 'chosen' CHECK (status IN ('chosen', 'candidate', 'rejected')),
  canonical_id  INTEGER REFERENCES media(id),  -- candidate/rejected -> the chosen row
  source        TEXT,                          -- tmdb | thetvdb | ... | merged
  raw_json      TEXT,                          -- raw response (pruned per raw_max_age_days)
  fetched_at    TEXT,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX media_kind_year ON media(kind, year);
CREATE INDEX media_parent ON media(parent_id, season, episode);
CREATE INDEX media_canonical ON media(canonical_id);

-- People and organizations as full entries ("Johnny Depp", "Studio Ghibli").
CREATE TABLE entities (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL,                 -- person | organization | character
  name          TEXT NOT NULL,
  original_name TEXT,
  sort_name     TEXT,
  born          TEXT,                          -- or founded
  died          TEXT,                          -- or dissolved
  status        TEXT NOT NULL DEFAULT 'chosen' CHECK (status IN ('chosen', 'candidate', 'rejected')),
  canonical_id  INTEGER REFERENCES entities(id),
  source        TEXT,
  raw_json      TEXT,
  fetched_at    TEXT,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX entities_name ON entities(name COLLATE NOCASE);

CREATE TABLE media_entities (
  media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  entity_id     INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
  role          TEXT NOT NULL,                 -- actor | director | writer | studio | network | ...
  character     TEXT NOT NULL DEFAULT '',
  ord           INTEGER,                       -- billing order
  source        TEXT,
  PRIMARY KEY (media_id, entity_id, role, character)
);
CREATE INDEX media_entities_entity ON media_entities(entity_id, role);

-- Ids in online databases, for media and entities.
CREATE TABLE external_ids (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER REFERENCES media(id) ON DELETE CASCADE,
  entity_id     INTEGER REFERENCES entities(id) ON DELETE CASCADE,
  source        TEXT NOT NULL,                 -- imdb | tmdb | thetvdb | wikidata | tvmaze | ...
  scope         TEXT NOT NULL DEFAULT '',      -- tmdb: movie | tv | episode | person | company
  value         TEXT NOT NULL,
  CHECK ((media_id IS NULL) != (entity_id IS NULL)),
  UNIQUE (source, scope, value)
);
CREATE INDEX external_ids_media ON external_ids(media_id);
CREATE INDEX external_ids_entity ON external_ids(entity_id);

-- Genres, normalized; parent_id allows sub-genres (horror > vampire).
CREATE TABLE genres (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE COLLATE NOCASE,
  parent_id     INTEGER REFERENCES genres(id)
);
CREATE TABLE media_genres (
  media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  genre_id      INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  source        TEXT,
  PRIMARY KEY (media_id, genre_id)
);
CREATE INDEX media_genres_genre ON media_genres(genre_id);

-- Groupings / universes / franchises / collections (Marvel, Ghibli, Batman).
CREATE TABLE groups (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL DEFAULT 'universe', -- universe | franchise | collection
  parent_id     INTEGER REFERENCES groups(id),
  source        TEXT,
  UNIQUE (name, kind)
);
CREATE TABLE media_groups (
  media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  group_id      INTEGER NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  ord           INTEGER,                       -- order within the group
  source        TEXT,
  PRIMARY KEY (media_id, group_id)
);
CREATE INDEX media_groups_group ON media_groups(group_id);

-- Media-to-media links (sequel, remake, based_on, spin_off): the start of graph search.
CREATE TABLE relations (
  from_id       INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  to_id         INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  rel           TEXT NOT NULL,
  source        TEXT,
  PRIMARY KEY (from_id, to_id, rel)
);
CREATE INDEX relations_to ON relations(to_id, rel);

-- Titles, descriptions, taglines, subtitles, summaries: every language, every source.
CREATE TABLE texts (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER REFERENCES media(id) ON DELETE CASCADE,
  entity_id     INTEGER REFERENCES entities(id) ON DELETE CASCADE,
  field         TEXT NOT NULL,                 -- title | description | tagline | subtitle | summary | biography
  language      TEXT,
  source        TEXT NOT NULL,                 -- tmdb | ... | sammy
  text          TEXT NOT NULL,
  chosen        INTEGER NOT NULL DEFAULT 0,    -- the one written to files
  fetched_at    TEXT,
  CHECK ((media_id IS NULL) != (entity_id IS NULL))
);
CREATE INDEX texts_media ON texts(media_id, field, chosen);
CREATE INDEX texts_entity ON texts(entity_id, field);

-- Full-text search over texts (kept in sync by triggers).
CREATE VIRTUAL TABLE texts_fts USING fts5(text, content='texts', content_rowid='id');
CREATE TRIGGER texts_ai AFTER INSERT ON texts BEGIN
  INSERT INTO texts_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER texts_ad AFTER DELETE ON texts BEGIN
  INSERT INTO texts_fts(texts_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER texts_au AFTER UPDATE OF text ON texts BEGIN
  INSERT INTO texts_fts(texts_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO texts_fts(rowid, text) VALUES (new.id, new.text);
END;

-- All art, chosen or not. local_path: a raw download (pruned per raw_max_age_days).
CREATE TABLE art (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER REFERENCES media(id) ON DELETE CASCADE,
  entity_id     INTEGER REFERENCES entities(id) ON DELETE CASCADE,
  group_id      INTEGER REFERENCES groups(id) ON DELETE CASCADE,
  kind          TEXT NOT NULL,                 -- poster | backdrop | still | season | logo | profile
  source        TEXT NOT NULL,
  language      TEXT,
  width         INTEGER,
  height        INTEGER,
  url           TEXT,
  local_path    TEXT,
  sha256        TEXT,
  chosen        INTEGER NOT NULL DEFAULT 0,
  fetched_at    TEXT,
  CHECK ((media_id IS NOT NULL) + (entity_id IS NOT NULL) + (group_id IS NOT NULL) = 1)
);
CREATE INDEX art_media ON art(media_id, kind, chosen);

CREATE TABLE ratings (
  media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  source        TEXT NOT NULL,                 -- imdb | rotten_tomatoes | metacritic | tmdb
  value         REAL NOT NULL,
  scale         REAL NOT NULL,                 -- 10, 100, ...
  votes         INTEGER,
  fetched_at    TEXT,
  PRIMARY KEY (media_id, source)
);

-- Files on disk; hash lets a renamed or moved file be recognized.
CREATE TABLE files (
  id            INTEGER PRIMARY KEY,
  media_id      INTEGER REFERENCES media(id) ON DELETE SET NULL,
  path          TEXT NOT NULL UNIQUE,
  size          INTEGER,
  mtime         TEXT,
  hash          TEXT,
  tagged_at     TEXT                           -- last time the file was written from the database
);
CREATE INDEX files_hash ON files(hash);
CREATE INDEX files_media ON files(media_id);
