-- D1 SQLite schema for hwpx-policy-hub
-- PII-free: only structural metadata and recognition biases

CREATE TABLE IF NOT EXISTS contributions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  contributor_hash TEXT NOT NULL,  -- SHA-256 of browser fingerprint (anonymous)
  payload_json TEXT NOT NULL,      -- community policy JSON (forms + fields + biases)
  forms_count INTEGER DEFAULT 0,
  fields_count INTEGER DEFAULT 0,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS aggregated_forms (
  fingerprint TEXT PRIMARY KEY,
  row_count INTEGER,
  col_count INTEGER,
  header_tokens TEXT,  -- JSON array
  contributor_count INTEGER DEFAULT 1,
  first_seen_at TEXT DEFAULT (datetime('now')),
  updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS aggregated_fields (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  form_fingerprint TEXT REFERENCES aggregated_forms(fingerprint),
  cell_addr TEXT,
  label_text TEXT,
  label_hash TEXT,
  canonical_field TEXT,
  cell_role TEXT,
  confidence REAL DEFAULT 0.5,
  vote_count INTEGER DEFAULT 1,
  reward_sum REAL DEFAULT 0,
  updated_at TEXT DEFAULT (datetime('now')),
  UNIQUE(form_fingerprint, cell_addr)
);

CREATE TABLE IF NOT EXISTS aggregated_policies (
  family TEXT PRIMARY KEY,
  biases_json TEXT,   -- {tableKindBiases, rowKindBiases, cellRoleBiases}
  contributor_count INTEGER DEFAULT 1,
  updated_at TEXT DEFAULT (datetime('now'))
);

-- Index for quick lookup
CREATE INDEX IF NOT EXISTS idx_fields_fingerprint ON aggregated_fields(form_fingerprint);
CREATE INDEX IF NOT EXISTS idx_contributions_created ON contributions(created_at);
