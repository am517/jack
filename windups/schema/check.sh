#!/bin/sh
# Validate the schema and the example searches. Needs sqlite3 with FTS5.
set -eu
cd "$(dirname "$0")"
out=$(cat media.sql examples.sql | sqlite3 :memory:)
expected='-- movies with Johnny Depp
Pirates of the Caribbean: The Curse of the Black Pearl
-- comedies before 1920
The Cook
-- horror incl. sub-genres, mentioning lovers (full text)
Only Lovers Left Alive
-- genres of a movie, and movies of a genre (links work both ways)
Animation
4
-- the Ghibli universe
Spirited Away
-- candidates kept for movie 1
Pirates (wrong match)'
if [ "$out" = "$expected" ]; then echo "schema OK"; else printf 'MISMATCH:\n%s\n' "$out"; exit 1; fi
