-- Small per-windup state (e.g. Daniel's changes since Walter's last patrol; Walter's last patrol).
CREATE TABLE windup_state (
    windup TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (windup, key)
);
