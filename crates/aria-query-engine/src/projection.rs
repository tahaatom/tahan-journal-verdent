//! پروجکشن تحلیلی داشبورد — جدول خلاصه روزانه بازتولیدشدنی.
//!
//! این جدول‌ها منبع حقیقت نیستند و از `journal_trades` بازسازی می‌شوند؛
//! خارج از نسخه‌بندی اسکیمای canonical (نسخه ۱) قرار دارند چون همیشه
//! قابل حذف و بازتولید کامل هستند.

use crate::error::QueryError;
use rusqlite::Connection;

/// زمان فعلی ISO-8601 UTC.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// نام جدول خلاصه روزانه پروجکشن.
pub const DAILY_SUMMARY_TABLE: &str = "query_trade_daily_summary";
/// نام جدول فراداده وضعیت پروجکشن.
pub const META_TABLE: &str = "query_projection_meta";

/// یک ردیف خلاصه روزانه داشبورد.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailySummaryRow {
    /// روز (YYYY-MM-DD) به مبنای COALESCE(exit_time, entry_time, created_at)
    pub day: String,
    pub account_id: String,
    pub trades_count: i64,
    pub wins: i64,
    pub losses: i64,
    pub pnl: Option<f64>,
    pub r_sum: Option<f64>,
}

/// اطمینان از وجود جدول‌های پروجکشن (بازتولیدشدنی — خارج از اسکیمای canonical).
pub fn ensure_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {DAILY_SUMMARY_TABLE} (
            day          TEXT NOT NULL,
            account_id   TEXT NOT NULL,
            trades_count INTEGER NOT NULL,
            wins         INTEGER NOT NULL,
            losses       INTEGER NOT NULL,
            pnl          REAL,
            r_sum        REAL,
            PRIMARY KEY (day, account_id)
        );
        CREATE TABLE IF NOT EXISTS {META_TABLE} (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    ))
}

/// خواندن یک کلید از فراداده پروجکشن (در نبود ردیف → None).
fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        &format!("SELECT value FROM {META_TABLE} WHERE key = ?1"),
        rusqlite::params![key],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// آیا پروجکشن کهنه است؟ (هرگز بازسازی نشده یا بعد از آخرین بازسازی باطل شده)
pub fn is_stale(conn: &Connection) -> Result<bool, QueryError> {
    ensure_tables(conn)?;
    Ok(meta_get(conn, "stale")?.as_deref() != Some("0"))
}

/// نسخه فعلی پروجکشن (برای آزمون و همبستگی رویداد).
pub fn version(conn: &Connection) -> Result<i64, QueryError> {
    ensure_tables(conn)?;
    Ok(meta_get(conn, "version")?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0))
}

/// خواندن عمومی یک کلید فراداده (مثل refreshed_at برای گزارش داشبورد).
pub fn meta_value(conn: &Connection, key: &str) -> Result<Option<String>, QueryError> {
    ensure_tables(conn)?;
    Ok(meta_get(conn, key)?)
}

/// باطل‌سازی پروجکشن — نسخه بالا می‌رود و پرچم کهنگی فعال می‌شود.
pub fn invalidate(conn: &Connection) -> Result<(), QueryError> {
    ensure_tables(conn)?;
    let v = version(conn)? + 1;
    let now = now_iso();
    conn.execute(
        &format!(
            "INSERT INTO {META_TABLE} (key, value) VALUES ('version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1"
        ),
        rusqlite::params![v.to_string()],
    )?;
    conn.execute(
        &format!(
            "INSERT INTO {META_TABLE} (key, value) VALUES ('stale', '1')
             ON CONFLICT(key) DO UPDATE SET value = '1'"
        ),
        [],
    )?;
    conn.execute(
        &format!(
            "INSERT INTO {META_TABLE} (key, value) VALUES ('invalidated_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1"
        ),
        rusqlite::params![now],
    )?;
    tracing::debug!(version = v, "پروجکشن داشبورد باطل شد");
    Ok(())
}

/// بازسازی کامل خلاصه روزانه از داده‌های زنده — در یک تراکنش اتمیک.
pub fn refresh(conn: &mut Connection) -> Result<(), QueryError> {
    ensure_tables(conn)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(&format!("DELETE FROM {DAILY_SUMMARY_TABLE}"), [])?;
    tx.execute(
        &format!(
            "INSERT INTO {DAILY_SUMMARY_TABLE}
                (day, account_id, trades_count, wins, losses, pnl, r_sum)
             SELECT
                substr(COALESCE(exit_time, entry_time, created_at), 1, 10),
                account_id,
                COUNT(*),
                SUM(CASE WHEN realized_pnl > 0 THEN 1 ELSE 0 END),
                SUM(CASE WHEN realized_pnl < 0 THEN 1 ELSE 0 END),
                SUM(realized_pnl),
                SUM(realized_r)
             FROM journal_trades
             WHERE deleted_at IS NULL
             GROUP BY 1, 2"
        ),
        [],
    )?;
    let now = now_iso();
    tx.execute(
        &format!(
            "INSERT INTO {META_TABLE} (key, value) VALUES ('stale', '0')
             ON CONFLICT(key) DO UPDATE SET value = '0'"
        ),
        [],
    )?;
    tx.execute(
        &format!(
            "INSERT INTO {META_TABLE} (key, value) VALUES ('refreshed_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1"
        ),
        rusqlite::params![now],
    )?;
    tx.commit()?;
    tracing::debug!("پروجکشن داشبورد بازسازی شد");
    Ok(())
}

/// اگر کهنه باشد بازسازی می‌کند؛ خروجی: آیا بازسازی انجام شد؟
pub fn refresh_if_stale(conn: &mut Connection) -> Result<bool, QueryError> {
    if is_stale(conn)? {
        refresh(conn)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// خواندن ردیف‌های خلاصه (با فیلتر اختیاری حساب).
pub fn read_summary(
    conn: &Connection,
    account_id: Option<&str>,
) -> Result<Vec<DailySummaryRow>, QueryError> {
    ensure_tables(conn)?;
    let (sql, params): (String, Vec<rusqlite::types::Value>) = match account_id {
        Some(acc) => (
            format!(
                "SELECT day, account_id, trades_count, wins, losses, pnl, r_sum
                 FROM {DAILY_SUMMARY_TABLE} WHERE account_id = ?1 ORDER BY day"
            ),
            vec![rusqlite::types::Value::Text(acc.to_string())],
        ),
        None => (
            format!(
                "SELECT day, account_id, trades_count, wins, losses, pnl, r_sum
                 FROM {DAILY_SUMMARY_TABLE} ORDER BY day"
            ),
            vec![],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params))?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(DailySummaryRow {
            day: r.get(0)?,
            account_id: r.get(1)?,
            trades_count: r.get(2)?,
            wins: r.get(3)?,
            losses: r.get(4)?,
            pnl: r.get(5)?,
            r_sum: r.get(6)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aria_storage_engine::{Database, migrations};

    fn db() -> Database {
        let db = Database::open_memory(Some("proj-pass-1")).unwrap();
        {
            let mut conn = db.lock();
            migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
            conn.execute(
                "INSERT INTO profiles (id, name, created_at, updated_at)
                 VALUES ('p1', 'پروفایل آزمون', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        }
        db
    }

    fn insert_trade(conn: &Connection, id: &str, account: &str, day: &str, pnl: Option<f64>, r: Option<f64>) {
        conn.execute(
            "INSERT OR IGNORE INTO trading_accounts (id, profile_id, name, created_at, updated_at)
             VALUES (?1, 'p1', 'حساب تست', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols (id, name, created_at) VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
            rusqlite::params![format!("sym-{id}"), format!("سیمبول-{id}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO journal_trades
                (id, account_id, symbol_id, direction, status, entry_time, exit_time,
                 realized_pnl, realized_r, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'buy', 'closed', ?4, ?4, ?5, ?6,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![id, account, format!("sym-{id}"), day, pnl, r],
        )
        .unwrap();
    }

    #[test]
    fn initial_state_is_stale() {
        let db = db();
        let conn = db.lock();
        assert!(is_stale(&conn).unwrap());
        assert_eq!(version(&conn).unwrap(), 0);
    }

    #[test]
    fn refresh_builds_daily_summary_and_clears_stale() {
        let db = db();
        {
            let mut conn = db.lock();
            insert_trade(&conn, "t1", "acc-main", "2026-03-01T10:00:00Z", Some(100.0), Some(2.0));
            insert_trade(&conn, "t2", "acc-main", "2026-03-01T15:00:00Z", Some(-50.0), Some(-1.0));
            insert_trade(&conn, "t3", "acc-main", "2026-03-02T10:00:00Z", Some(25.0), Some(1.0));
            refresh(&mut conn).unwrap();
        }
        let conn = db.lock();
        assert!(!is_stale(&conn).unwrap());
        let rows = read_summary(&conn, None).unwrap();
        assert_eq!(rows.len(), 2);
        let day1 = &rows[0];
        assert_eq!(day1.day, "2026-03-01");
        assert_eq!(day1.trades_count, 2);
        assert_eq!(day1.wins, 1);
        assert_eq!(day1.losses, 1);
        assert!((day1.pnl.unwrap() - 50.0).abs() < 1e-9);
        assert!((day1.r_sum.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn invalidate_bumps_version_and_marks_stale() {
        let db = db();
        {
            let mut conn = db.lock();
            insert_trade(&conn, "t1", "acc-main", "2026-03-01T10:00:00Z", Some(100.0), Some(2.0));
            refresh(&mut conn).unwrap();
        }
        let conn = db.lock();
        assert!(!is_stale(&conn).unwrap());
        invalidate(&conn).unwrap();
        assert_eq!(version(&conn).unwrap(), 1);
        assert!(is_stale(&conn).unwrap());
        // داده قدیمی خوانده می‌شود تا بازسازی بعدی
        assert_eq!(read_summary(&conn, None).unwrap().len(), 1);
    }

    #[test]
    fn refresh_if_stale_is_idempotent() {
        let db = db();
        {
            let mut conn = db.lock();
            insert_trade(&conn, "t1", "acc-main", "2026-03-01T10:00:00Z", Some(100.0), Some(2.0));
            refresh(&mut conn).unwrap();
        }
        let mut conn = db.lock();
        assert!(!refresh_if_stale(&mut conn).unwrap());
        invalidate(&conn).unwrap();
        assert!(refresh_if_stale(&mut conn).unwrap());
        assert!(!refresh_if_stale(&mut conn).unwrap());
    }

    #[test]
    fn account_filter_works() {
        let db = db();
        {
            let mut conn = db.lock();
            insert_trade(&conn, "t1", "acc-a", "2026-03-01T10:00:00Z", Some(100.0), Some(2.0));
            insert_trade(&conn, "t2", "acc-b", "2026-03-01T11:00:00Z", Some(200.0), Some(3.0));
            refresh(&mut conn).unwrap();
        }
        let conn = db.lock();
        let rows = read_summary(&conn, Some("acc-a")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].account_id, "acc-a");
        assert_eq!(rows[0].trades_count, 1);
    }
}
