-- The team's public page lives at `{slug}.TEAM_DOMAIN` (e.g. acme.localhost).
ALTER TABLE teams ADD COLUMN slug TEXT NOT NULL DEFAULT '';
UPDATE teams SET slug = 'team-' || id;
CREATE UNIQUE INDEX teams_slug ON teams (slug);
