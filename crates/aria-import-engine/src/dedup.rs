//! تشخیص ردیف‌های تکراری ایمپورت (فاز ۱.۱۵).
//!
//! دو سطح تشخیص طبق قرارداد فاز:
//! ۱) هش blake3 ردیف خام در `source_records` — ایمپورت مجدد همان فایل؛
//! ۲) شماره تیکت بروکر در `executions` — فایل متفاوت حاوی همان معامله.

use crate::error::ImportError;
use crate::model::MappedDeal;
use rusqlite::Connection;

/// آیا این ردیف قبلاً ایمپورت شده است؟
pub fn is_row_duplicate(conn: &Connection, deal: &MappedDeal) -> Result<bool, ImportError> {
    let hash_hit: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM source_records WHERE payload_hash = ?1)",
            [&deal.payload_hash],
            |r| r.get(0),
        )
        .map_err(|e| ImportError::Storage(e.to_string()))?;
    if hash_hit {
        return Ok(true);
    }
    if let Some(ticket) = &deal.ticket {
        let ticket_hit: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM executions WHERE ticket = ?1)",
                [ticket],
                |r| r.get(0),
            )
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        if ticket_hit {
            return Ok(true);
        }
    }
    Ok(false)
}

/// آیا کل فایل تکراری است؟ هیچ معامله جدید، هیچ خطا و همه ردیف‌های
/// *معاملاتی* تکراری. `deal_rows` = تعداد ردیف‌های معاملاتی (بدون
/// ردیف‌های غیرمعاملاتی رد‌شده) تا فایل صورت‌حساب با ردیف موجودی هم
/// درست تشخیص داده شود.
pub fn is_file_duplicate(deal_rows: usize, imported: usize, duplicates: usize, errors: usize) -> bool {
    deal_rows > 0 && errors == 0 && imported == 0 && duplicates >= deal_rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DealRole, DealSide};

    fn conn_with_schema() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(aria_storage_engine::schema_v1::SCHEMA_V1).unwrap();
        conn
    }

    fn deal(ticket: Option<&str>) -> MappedDeal {
        MappedDeal {
            row_index: 2,
            raw: "raw,line".into(),
            payload_hash: crate::parse::row_hash("raw,line"),
            ticket: ticket.map(|t| t.into()),
            position_id: Some("100".into()),
            symbol: "XAUUSD".into(),
            direction: DealSide::Buy,
            role: DealRole::In,
            time: "2024-01-15T10:30:00Z".into(),
            close_time: None,
            price: 2035.5,
            volume: 0.1,
            commission: 0.0,
            swap: 0.0,
            profit: 0.0,
            magic: None,
            comment: None,
        }
    }

    #[test]
    fn fresh_row_is_not_duplicate() {
        let conn = conn_with_schema();
        assert!(!is_row_duplicate(&conn, &deal(Some("771"))).unwrap());
    }

    #[test]
    fn same_hash_in_source_records_is_duplicate() {
        let conn = conn_with_schema();
        conn.execute(
            "INSERT INTO source_records (id, import_batch_id, raw_payload, payload_hash, source, imported_at, processed_status)
             VALUES ('sr1','b1','raw,line','H1','mt5','2024-01-15T10:30:00Z','processed')",
            [],
        )
        .unwrap();
        let d = deal(Some("771"));
        let d = MappedDeal { payload_hash: "H1".into(), ..d };
        assert!(is_row_duplicate(&conn, &d).unwrap());
    }

    #[test]
    fn same_ticket_in_executions_is_duplicate() {
        let conn = conn_with_schema();
        conn.execute(
            "INSERT INTO executions (id, kind, direction, price, volume, ticket, assignment_status, created_at)
             VALUES ('e1','imported','buy',2035.5,0.1,'771','assigned','2024-01-15T10:30:00Z')",
            [],
        )
        .unwrap();
        assert!(is_row_duplicate(&conn, &deal(Some("771"))).unwrap());
        assert!(!is_row_duplicate(&conn, &deal(Some("772"))).unwrap());
    }

    #[test]
    fn row_without_ticket_only_matches_by_hash() {
        let conn = conn_with_schema();
        assert!(!is_row_duplicate(&conn, &deal(None)).unwrap());
    }

    #[test]
    fn file_duplicate_requires_all_duplicate_and_no_errors() {
        assert!(is_file_duplicate(3, 0, 3, 0));
        assert!(!is_file_duplicate(3, 1, 2, 0), "imported rows exist");
        assert!(!is_file_duplicate(3, 0, 2, 1), "errors present");
        assert!(!is_file_duplicate(0, 0, 0, 0), "empty file is not a duplicate");
    }
}
