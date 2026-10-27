-- Optional: semantic search ("movies of gay vampires") with sqlite-vec (load the extension first).
-- One vector per texts row; dimension must match the embedding model (384: all-MiniLM-L6-v2 / bge-small).
CREATE VIRTUAL TABLE text_vectors USING vec0(text_id INTEGER PRIMARY KEY, embedding float[384]);
