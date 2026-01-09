PRAGMA foreign_keys = ON;

CREATE TABLE app_settings_text (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
