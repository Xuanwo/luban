PRAGMA foreign_keys = ON;

CREATE TABLE app_settings (
  key        TEXT PRIMARY KEY,
  value      INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
