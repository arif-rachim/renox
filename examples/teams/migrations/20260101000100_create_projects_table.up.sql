CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    team_id INTEGER NOT NULL REFERENCES teams (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT,
    updated_at TEXT
);
-- Names are unique per team; the validation rule checks the same thing first.
CREATE UNIQUE INDEX projects_team_id_name ON projects (team_id, name);
