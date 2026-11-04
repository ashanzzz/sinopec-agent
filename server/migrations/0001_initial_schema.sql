-- Sinopec Agent SQLite Schema (Local operational state only - Sinopec server is source of truth)

CREATE TABLE IF NOT EXISTS operations (
    id TEXT PRIMARY KEY NOT NULL,
    operation_type TEXT NOT NULL,
    phase TEXT NOT NULL,
    state TEXT NOT NULL,
    parameters_json TEXT NOT NULL,
    idempotency_key TEXT UNIQUE,
    request_hash TEXT,
    remote_id TEXT,
    remote_result_json TEXT,
    error_code TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_operations_idempotency ON operations(idempotency_key);
CREATE INDEX IF NOT EXISTS idx_operations_state ON operations(state);

CREATE TABLE IF NOT EXISTS human_actions (
    id TEXT PRIMARY KEY NOT NULL,
    token TEXT UNIQUE NOT NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL,
    message TEXT NOT NULL,
    operation_id TEXT,
    viewer_url TEXT,
    context_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_human_actions_token ON human_actions(token);
CREATE INDEX IF NOT EXISTS idx_human_actions_status ON human_actions(status);

CREATE TABLE IF NOT EXISTS downloads (
    id TEXT PRIMARY KEY NOT NULL,
    remote_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size INTEGER NOT NULL,
    downloaded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_downloads_remote_id ON downloads(remote_id);

CREATE TABLE IF NOT EXISTS research_state (
    id TEXT PRIMARY KEY NOT NULL,
    endpoint_name TEXT NOT NULL,
    account_mode TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    status TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    last_verified_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS admin_users (
    username TEXT PRIMARY KEY NOT NULL,
    password_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_login_at TEXT
);

CREATE TABLE IF NOT EXISTS local_quancun_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_no TEXT NOT NULL,
    card_alias TEXT,
    holder_name TEXT,
    ope_time TEXT NOT NULL,
    amount_fen INTEGER NOT NULL,
    amount_yuan TEXT NOT NULL,
    balance_yuan TEXT NOT NULL,
    node_tag TEXT NOT NULL,
    tra_name TEXT NOT NULL,
    oil_name TEXT,
    synced_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_quancun_unique ON local_quancun_records(card_no, ope_time, amount_fen);

CREATE TABLE IF NOT EXISTS local_invoices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_no TEXT UNIQUE NOT NULL,
    invoice_number TEXT,
    einv_req_key TEXT,
    title TEXT,
    amount_fen INTEGER NOT NULL,
    amount_yuan TEXT NOT NULL,
    fplx TEXT,
    add_date TEXT NOT NULL,
    status INTEGER,
    url TEXT,
    start_date TEXT,
    end_date TEXT,
    system TEXT,
    synced_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS local_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    master_card_no TEXT NOT NULL,
    card_no TEXT NOT NULL,
    card_alias TEXT,
    holder_name TEXT,
    ope_time TEXT NOT NULL,
    amount_fen INTEGER NOT NULL,
    amount_yuan TEXT NOT NULL,
    node_name TEXT NOT NULL,
    status_text TEXT NOT NULL,
    synced_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_allocations_unique ON local_allocations(card_no, ope_time, amount_fen);

CREATE TABLE IF NOT EXISTS local_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_no TEXT NOT NULL,
    card_alias TEXT,
    holder_name TEXT,
    ope_time TEXT NOT NULL,
    amount_fen INTEGER NOT NULL,
    amount_yuan TEXT NOT NULL,
    balance_yuan TEXT NOT NULL,
    oil_name TEXT,
    price_yuan TEXT,
    volume_litre TEXT,
    node_tag TEXT NOT NULL,
    tra_name TEXT NOT NULL,
    synced_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_unique ON local_transactions(card_no, ope_time, amount_fen, balance_yuan);


