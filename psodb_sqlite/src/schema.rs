pub static SCHEMA: &'static str = "
BEGIN;

CREATE TABLE IF NOT EXISTS version (
    version INTEGER PRIMARY KEY
);
INSERT OR REPLACE INTO version (version) VALUES (0);

CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    password_invalidated INTEGER NOT NULL DEFAULT 0,
    banned INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS bb_guildcard (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL DEFAULT 400000000,
    account_id INTEGER UNIQUE NOT NULL,
    data BLOB NOT NULL
);

COMMIT;
";

//pub static MIGRATIONS: [&'static str] = [""];
