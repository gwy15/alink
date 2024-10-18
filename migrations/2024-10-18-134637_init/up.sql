-- Your SQL goes here
CREATE TABLE `links` (
    `id`            INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
    `src`           TEXT        NOT NULL,
    `target`        TEXT        NOT NULL,
    `created_at`    TIMESTAMP   NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`    TIMESTAMP   NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (`src`) ON CONFLICT REPLACE
);
CREATE INDEX `idx_src` ON `links` (`src`);
