CREATE TABLE IF NOT EXISTS visits (
    url TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    count INTEGER NOT NULL DEFAULT 0,
    last_visit INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS visits_fts USING fts5(title, url, content='visits', content_rowid='rowid');

CREATE TRIGGER IF NOT EXISTS visits_ai AFTER INSERT ON visits BEGIN
    INSERT INTO visits_fts(rowid, title, url) VALUES (new.rowid, new.title, new.url);
END;

CREATE TRIGGER IF NOT EXISTS visits_ad AFTER DELETE ON visits BEGIN
    INSERT INTO visits_fts(visits_fts, rowid, title, url) VALUES('delete', old.rowid, old.title, old.url);
END;

CREATE TRIGGER IF NOT EXISTS visits_au AFTER UPDATE ON visits BEGIN
    INSERT INTO visits_fts(visits_fts, rowid, title, url) VALUES('delete', old.rowid, old.title, old.url);
    INSERT INTO visits_fts(rowid, title, url) VALUES (new.rowid, new.title, new.url);
END;

CREATE TABLE IF NOT EXISTS bookmarks (
    url TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    created INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS session (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    json TEXT NOT NULL
);
