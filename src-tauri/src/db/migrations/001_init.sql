-- Categories (one per sound) and collections (many-to-many) share a table and sidebar UI.
CREATE TABLE groups (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('category', 'collection')),
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    icon        TEXT NOT NULL DEFAULT 'folder',
    color       TEXT NOT NULL DEFAULT '#8B8D98',
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
CREATE INDEX idx_groups_kind_position ON groups (kind, position);

CREATE TABLE sounds (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    file_name       TEXT NOT NULL UNIQUE,          -- relative to library/sounds
    original_name   TEXT NOT NULL,
    format          TEXT NOT NULL,
    duration_ms     INTEGER NOT NULL,
    file_size       INTEGER NOT NULL,
    sample_rate     INTEGER NOT NULL,
    channels        INTEGER NOT NULL,
    cover_file      TEXT,                          -- relative to library/covers
    volume          REAL NOT NULL DEFAULT 1.0 CHECK (volume BETWEEN 0 AND 2),
    trim_start_ms   INTEGER NOT NULL DEFAULT 0 CHECK (trim_start_ms >= 0),
    trim_end_ms     INTEGER,                       -- NULL plays to the end
    playback_mode   TEXT NOT NULL DEFAULT 'default'
                    CHECK (playback_mode IN ('default', 'restart', 'toggle', 'overlap', 'hold')),
    category_id     TEXT REFERENCES groups (id) ON DELETE SET NULL,
    is_favorite     INTEGER NOT NULL DEFAULT 0,
    is_pinned       INTEGER NOT NULL DEFAULT 0,
    play_count      INTEGER NOT NULL DEFAULT 0,
    last_played_at  INTEGER,
    position        INTEGER NOT NULL DEFAULT 0,
    shortcut        TEXT UNIQUE,
    waveform        BLOB NOT NULL,
    source_url      TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
CREATE INDEX idx_sounds_category ON sounds (category_id);
CREATE INDEX idx_sounds_position ON sounds (position);
CREATE INDEX idx_sounds_last_played ON sounds (last_played_at);

CREATE TABLE collection_items (
    collection_id  TEXT NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
    sound_id       TEXT NOT NULL REFERENCES sounds (id) ON DELETE CASCADE,
    added_at       INTEGER NOT NULL,
    PRIMARY KEY (collection_id, sound_id)
);
CREATE INDEX idx_collection_items_sound ON collection_items (sound_id);

CREATE TABLE downloads (
    id             TEXT PRIMARY KEY,
    url            TEXT NOT NULL,
    source         TEXT NOT NULL,
    title          TEXT,
    thumbnail_url  TEXT,
    duration_ms    INTEGER,
    status         TEXT NOT NULL
                   CHECK (status IN ('downloading', 'ready', 'imported', 'failed', 'cancelled')),
    error          TEXT,
    temp_file      TEXT,                           -- relative to tmp/ while status = 'ready'
    sound_id       TEXT REFERENCES sounds (id) ON DELETE SET NULL,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);
CREATE INDEX idx_downloads_created ON downloads (created_at DESC);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);
