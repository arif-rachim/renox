-- The language each user reads mail and notifications in (Renox reads a
-- `locale` column for `Recipient::locale`); set by the language switch.
ALTER TABLE users ADD COLUMN locale TEXT;
