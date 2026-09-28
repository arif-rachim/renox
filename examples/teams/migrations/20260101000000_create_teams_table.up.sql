CREATE TABLE teams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    -- Sealed with state.encrypt (AES-256-GCM under APP_KEY), never plain text.
    webhook_secret TEXT,
    created_at TEXT,
    updated_at TEXT
);

-- Memberships: who is in which team, and as what (owner or member).
CREATE TABLE team_user (
    team_id INTEGER NOT NULL REFERENCES teams (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member',
    created_at TEXT,
    updated_at TEXT,
    PRIMARY KEY (team_id, user_id)
);
CREATE INDEX team_user_user_id ON team_user (user_id);
