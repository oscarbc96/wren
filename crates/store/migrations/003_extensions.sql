CREATE TABLE IF NOT EXISTS extensions (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    path TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1
);
