-- Sample data plus the kinds of searches Steve will need. Run by check.sh.
INSERT INTO media(id, kind, title, year, source) VALUES
  (1, 'movie', 'Pirates of the Caribbean: The Curse of the Black Pearl', 2003, 'tmdb'),
  (2, 'movie', 'The Kid', 1921, 'tmdb'),
  (3, 'movie', 'One Week', 1920, 'tmdb'),
  (4, 'movie', 'The Cook', 1918, 'tmdb'),
  (5, 'movie', 'Only Lovers Left Alive', 2013, 'tmdb'),
  (6, 'movie', 'Spirited Away', 2001, 'tmdb'),
  (7, 'movie', 'Pirates (wrong match)', 1986, 'tmdb');
UPDATE media SET status = 'candidate', canonical_id = 1 WHERE id = 7;
INSERT INTO entities(id, kind, name) VALUES (1, 'person', 'Johnny Depp'), (2, 'organization', 'Studio Ghibli');
INSERT INTO media_entities(media_id, entity_id, role, character, ord) VALUES (1, 1, 'actor', 'Jack Sparrow', 1), (6, 2, 'studio', '', NULL);
INSERT INTO external_ids(media_id, source, scope, value) VALUES (1, 'tmdb', 'movie', '22'), (1, 'imdb', '', 'tt0325980');
INSERT INTO external_ids(entity_id, source, scope, value) VALUES (1, 'tmdb', 'person', '85');
INSERT INTO genres(id, name) VALUES (1, 'Comedy'), (2, 'Horror'), (3, 'Animation');
INSERT INTO genres(id, name, parent_id) VALUES (4, 'Vampire', 2);
INSERT INTO media_genres(media_id, genre_id) VALUES (1, 1), (2, 1), (3, 1), (4, 1), (5, 4), (6, 3);
INSERT INTO groups(id, name) VALUES (1, 'Ghibli');
INSERT INTO media_groups(media_id, group_id) VALUES (6, 1);
INSERT INTO texts(media_id, field, language, source, text, chosen) VALUES
  (5, 'description', 'en', 'tmdb', 'Two centuries-old vampire lovers reunite in Detroit.', 1),
  (5, 'description', 'en', 'thetvdb', 'Vampires in love.', 0);

.mode list
SELECT '-- movies with Johnny Depp';
SELECT m.title FROM media m JOIN media_entities me ON me.media_id = m.id JOIN entities e ON e.id = me.entity_id
  WHERE e.name = 'Johnny Depp' AND m.status = 'chosen';
SELECT '-- comedies before 1920';
SELECT m.title FROM media m JOIN media_genres mg ON mg.media_id = m.id JOIN genres g ON g.id = mg.genre_id
  WHERE g.name = 'comedy' AND m.year < 1920 AND m.status = 'chosen';
SELECT '-- horror incl. sub-genres, mentioning lovers (full text)';
WITH RECURSIVE g(id) AS (SELECT id FROM genres WHERE name = 'Horror' UNION SELECT genres.id FROM genres JOIN g ON genres.parent_id = g.id)
SELECT DISTINCT m.title FROM media m JOIN media_genres mg ON mg.media_id = m.id JOIN texts t ON t.media_id = m.id
  WHERE mg.genre_id IN (SELECT id FROM g) AND t.id IN (SELECT rowid FROM texts_fts WHERE texts_fts MATCH 'lovers');
SELECT '-- genres of a movie, and movies of a genre (links work both ways)';
SELECT g.name FROM genres g JOIN media_genres mg ON mg.genre_id = g.id WHERE mg.media_id = 6;
SELECT count(*) FROM media_genres WHERE genre_id = 1;
SELECT '-- the Ghibli universe';
SELECT m.title FROM media m JOIN media_groups mg ON mg.media_id = m.id JOIN groups gr ON gr.id = mg.group_id WHERE gr.name = 'Ghibli';
SELECT '-- candidates kept for movie 1';
SELECT title FROM media WHERE canonical_id = 1;
