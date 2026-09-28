//! اسکیمای فیزیکی نسخه ۱ — منبع حقیقت ساختار پایگاه‌داده.
//!
//! مرجع مستند: `docs/database/physical-schema-v1.md`
//! قواعد:
//! - همه زمان‌ها UTC/ISO 8601 (TEXT).
//! - داده خام (source_records) تغییرناپذیر است.
//! - فیلدهای رزرو نسخه ۱ در journal_trades از همین ابتدا موجودند.
//! - projections فقط کش بازتولیدشدنی هستند.

pub const SCHEMA_V1: &str = r#"
-- ===================== پروفایل و حساب‌ها =====================
CREATE TABLE IF NOT EXISTS profiles (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS trading_accounts (
    id            TEXT PRIMARY KEY,
    profile_id    TEXT NOT NULL REFERENCES profiles(id),
    name          TEXT NOT NULL,
    broker        TEXT,
    account_type  TEXT,
    currency      TEXT NOT NULL DEFAULT 'USD',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_accounts_profile ON trading_accounts(profile_id);

CREATE TABLE IF NOT EXISTS symbols (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL UNIQUE,
    description    TEXT,
    contract_size  REAL NOT NULL DEFAULT 1,
    pip_size       REAL,
    created_at     TEXT NOT NULL
);

-- ===================== ژورنال معامله =====================
CREATE TABLE IF NOT EXISTS journal_trades (
    id            TEXT PRIMARY KEY,
    account_id    TEXT NOT NULL REFERENCES trading_accounts(id),
    symbol_id     TEXT NOT NULL REFERENCES symbols(id),
    direction     TEXT NOT NULL CHECK (direction IN ('buy','sell')),
    status        TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','closed','cancelled')),
    strategy      TEXT,
    timeframe     TEXT,
    session       TEXT,
    market_condition TEXT,
    entry_type    TEXT,
    note          TEXT,
    tags          TEXT,
    emotions      TEXT,
    mistakes      TEXT,
    entry_time    TEXT,
    exit_time     TEXT,
    initial_stop_loss REAL,
    take_profit   REAL,
    manual_risk   REAL,
    -- محاسبات R
    risk_calculation_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (risk_calculation_status IN ('pending','calculated','no_stop_loss','manual_risk','needs_assignment')),
    risk_basis    TEXT CHECK (risk_basis IS NULL OR risk_basis IN ('initial_stop_loss','manual_risk')),
    planned_r     REAL,
    initial_risk_amount REAL,
    realized_pnl  REAL,
    realized_r    REAL,
    trade_r       REAL,
    commission    REAL NOT NULL DEFAULT 0,
    swap          REAL NOT NULL DEFAULT 0,
    -- رزرو نسخه ۱ (فعال در نسخه ۳)
    position_group_id       TEXT,
    mae_price     REAL,
    mfe_price     REAL,
    mae_amount    REAL,
    mfe_amount    REAL,
    mae_r         REAL,
    mfe_r         REAL,
    max_drawdown_inside_trade REAL,
    -- حذف نرم — داده کاربر هرگز سخت‌حذف نمی‌شود
    deleted_at    TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_trades_account ON journal_trades(account_id);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON journal_trades(symbol_id);
CREATE INDEX IF NOT EXISTS idx_trades_status ON journal_trades(status);
CREATE INDEX IF NOT EXISTS idx_trades_entry_time ON journal_trades(entry_time);
CREATE INDEX IF NOT EXISTS idx_trades_account_time ON journal_trades(account_id, entry_time);
CREATE INDEX IF NOT EXISTS idx_trades_symbol_time ON journal_trades(symbol_id, entry_time);

-- ===================== پاها =====================
CREATE TABLE IF NOT EXISTS entry_legs (
    id             TEXT PRIMARY KEY,
    trade_id       TEXT NOT NULL REFERENCES journal_trades(id) ON DELETE CASCADE,
    planned_price  REAL,
    executed_price REAL,
    volume         REAL NOT NULL CHECK (volume > 0),
    stop_loss      REAL,
    take_profit    REAL,
    entry_time     TEXT,
    note           TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entry_legs_trade ON entry_legs(trade_id);

CREATE TABLE IF NOT EXISTS exit_legs (
    id             TEXT PRIMARY KEY,
    trade_id       TEXT NOT NULL REFERENCES journal_trades(id) ON DELETE CASCADE,
    exit_reason    TEXT,
    executed_price REAL,
    volume         REAL NOT NULL CHECK (volume > 0),
    exit_time      TEXT,
    note           TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_exit_legs_trade ON exit_legs(trade_id);

-- ===================== اجراها =====================
CREATE TABLE IF NOT EXISTS executions (
    id                TEXT PRIMARY KEY,
    trade_id          TEXT REFERENCES journal_trades(id) ON DELETE CASCADE,
    leg_id            TEXT,
    leg_kind          TEXT CHECK (leg_kind IS NULL OR leg_kind IN ('entry','exit')),
    source_record_id  TEXT REFERENCES source_records(id),
    kind              TEXT NOT NULL DEFAULT 'manual' CHECK (kind IN ('manual','imported')),
    direction         TEXT NOT NULL CHECK (direction IN ('buy','sell')),
    price             REAL NOT NULL,
    volume            REAL NOT NULL CHECK (volume > 0),
    executed_at       TEXT,
    commission        REAL NOT NULL DEFAULT 0,
    swap              REAL NOT NULL DEFAULT 0,
    ticket            TEXT,
    magic             TEXT,
    comment           TEXT,
    assignment_status TEXT NOT NULL DEFAULT 'assigned' CHECK (assignment_status IN ('assigned','needs_assignment')),
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_executions_trade ON executions(trade_id);
CREATE INDEX IF NOT EXISTS idx_executions_leg ON executions(leg_id);
CREATE INDEX IF NOT EXISTS idx_executions_assignment ON executions(assignment_status);
CREATE INDEX IF NOT EXISTS idx_executions_ticket ON executions(ticket);

-- ===================== داده خام منبع (غیرقابل تغییر) =====================
CREATE TABLE IF NOT EXISTS source_records (
    id                TEXT PRIMARY KEY,
    import_batch_id   TEXT NOT NULL,
    raw_payload       TEXT NOT NULL,
    payload_hash      TEXT NOT NULL,
    source            TEXT NOT NULL,
    imported_at       TEXT NOT NULL,
    processed_status  TEXT NOT NULL DEFAULT 'pending'
        CHECK (processed_status IN ('pending','processed','failed','duplicate'))
);
CREATE INDEX IF NOT EXISTS idx_source_records_batch ON source_records(import_batch_id, processed_status);
CREATE INDEX IF NOT EXISTS idx_source_records_hash ON source_records(payload_hash);

-- ===================== بازنویسی دستی =====================
CREATE TABLE IF NOT EXISTS manual_overrides (
    id             TEXT PRIMARY KEY,
    entity_type    TEXT NOT NULL,
    entity_id      TEXT NOT NULL,
    field_name     TEXT NOT NULL,
    previous_value TEXT,
    new_value      TEXT,
    reason         TEXT,
    source         TEXT NOT NULL,
    priority       INTEGER NOT NULL DEFAULT 0,
    reversible     INTEGER NOT NULL DEFAULT 1,
    created_by     TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    reverted_at    TEXT
);
CREATE INDEX IF NOT EXISTS idx_overrides_entity ON manual_overrides(entity_type, entity_id);

-- ===================== فیلدهای سفارشی =====================
CREATE TABLE IF NOT EXISTS custom_fields (
    id                 TEXT PRIMARY KEY,
    technical_key      TEXT NOT NULL UNIQUE,
    display_label      TEXT NOT NULL,
    description        TEXT,
    storage_type       TEXT NOT NULL CHECK (storage_type IN
        ('text','long_text','integer','decimal','boolean','datetime','enum','multi_enum','rating','tag_set')),
    semantic_type      TEXT NOT NULL CHECK (semantic_type IN
        ('short_text','long_text','number','price','percent','money','datetime','boolean','single_select','multi_select','rating','tag')),
    unit               TEXT,
    default_value      TEXT,
    required           INTEGER NOT NULL DEFAULT 0,
    active             INTEGER NOT NULL DEFAULT 1,
    filterable         INTEGER NOT NULL DEFAULT 0,
    stat_enabled       INTEGER NOT NULL DEFAULT 0,
    analysis_enabled   INTEGER NOT NULL DEFAULT 0,
    display_order      INTEGER NOT NULL DEFAULT 0,
    form_group         TEXT,
    validation_rules   TEXT,
    schema_version     INTEGER NOT NULL DEFAULT 1,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS field_options (
    id         TEXT PRIMARY KEY,
    field_id   TEXT NOT NULL REFERENCES custom_fields(id) ON DELETE CASCADE,
    value      TEXT NOT NULL,
    label      TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    active     INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_field_options_field ON field_options(field_id);

CREATE TABLE IF NOT EXISTS field_values (
    trade_id       TEXT NOT NULL REFERENCES journal_trades(id) ON DELETE CASCADE,
    field_id       TEXT NOT NULL REFERENCES custom_fields(id),
    text_value     TEXT,
    integer_value  INTEGER,
    decimal_value  REAL,
    boolean_value  INTEGER,
    datetime_value TEXT,
    json_value     TEXT,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (trade_id, field_id)
);
CREATE INDEX IF NOT EXISTS idx_field_values_field ON field_values(field_id);
CREATE INDEX IF NOT EXISTS idx_field_values_text ON field_values(field_id, text_value) WHERE text_value IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_field_values_int ON field_values(field_id, integer_value) WHERE integer_value IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_field_values_decimal ON field_values(field_id, decimal_value) WHERE decimal_value IS NOT NULL;

-- ===================== پیوست‌ها =====================
CREATE TABLE IF NOT EXISTS attachments (
    id             TEXT PRIMARY KEY,
    file_name      TEXT NOT NULL,
    file_path      TEXT NOT NULL,
    thumbnail_path TEXT,
    mime_type      TEXT,
    size_bytes     INTEGER NOT NULL,
    blake3_hash    TEXT NOT NULL,
    width          INTEGER,
    height         INTEGER,
    created_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_attachments_hash ON attachments(blake3_hash);

CREATE TABLE IF NOT EXISTS attachment_trade_links (
    id            TEXT PRIMARY KEY,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
    trade_id      TEXT NOT NULL REFERENCES journal_trades(id) ON DELETE CASCADE,
    link_kind     TEXT NOT NULL CHECK (link_kind IN ('before_trade','after_trade','chart','news','other')),
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_attach_links_trade ON attachment_trade_links(trade_id);
CREATE INDEX IF NOT EXISTS idx_attach_links_attachment ON attachment_trade_links(attachment_id);

-- ===================== حسابرسی و رویدادهای سیستم =====================
CREATE TABLE IF NOT EXISTS audit_logs (
    id         TEXT PRIMARY KEY,
    action     TEXT NOT NULL,
    actor      TEXT NOT NULL,
    target     TEXT,
    detail     TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_time ON audit_logs(created_at);

CREATE TABLE IF NOT EXISTS system_events (
    id             TEXT PRIMARY KEY,
    event_type     TEXT NOT NULL,
    event_version  INTEGER NOT NULL DEFAULT 1,
    source         TEXT NOT NULL,
    correlation_id TEXT,
    payload        TEXT NOT NULL,
    published      INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_system_events_published ON system_events(published, created_at);

-- ===================== پلاگین‌ها و تنظیمات =====================
CREATE TABLE IF NOT EXISTS plugins (
    id            TEXT PRIMARY KEY,
    manifest_json TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'installed'
        CHECK (status IN ('installed','enabled','running','stopped','crashed','quarantined','disabled')),
    crash_count   INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    installed_at  TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value      TEXT,
    updated_at TEXT NOT NULL
);

-- ===================== پروجکشن داشبورد (کش بازتولیدشدنی) =====================
CREATE TABLE IF NOT EXISTS trade_daily_summary (
    date         TEXT NOT NULL,
    account_id   TEXT NOT NULL,
    trades_count INTEGER NOT NULL DEFAULT 0,
    wins         INTEGER NOT NULL DEFAULT 0,
    losses       INTEGER NOT NULL DEFAULT 0,
    pnl          REAL NOT NULL DEFAULT 0,
    r_sum        REAL NOT NULL DEFAULT 0,
    updated_at   TEXT NOT NULL,
    PRIMARY KEY (date, account_id)
);

CREATE TABLE IF NOT EXISTS meta_projection_version (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;
