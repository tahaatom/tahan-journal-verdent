//! سرویس دامنه — اجرای اتمیک ۱۱ دستور، outbox رویداد، داده مؤثر و بازمحاسبه R.
//!
//! قواعد پیاده‌سازی:
//! - هر دستور در یک تراکنش اتمیک اجرا می‌شود؛ شکست یعنی بدون تغییر.
//! - رویدادها داخل همان تراکنش در outbox (`system_events`) نوشته و پس از commit
//!   به فراخواننده بازگردانده می‌شوند تا انتشار (broadcast) بعد از commit انجام گیرد.
//! - حذف معامله فقط نرم است؛ SourceRecord هرگز تغییر یا حذف نمی‌شود.
//! - تغییر داده‌های canonical قیمت/ریسک → بازمحاسبه خودکار R در همان تراکنش.

use crate::commands::*;
use crate::error::DomainError;
use crate::events::DomainEventFactory;
use crate::model::{parse_direction, Direction, EntryLeg, Execution, ExitLeg, ManualOverride, Trade, TradeStatus};
use crate::risk::{compute_trade_risk, EntryLegInput, ExitLegInput};
use aria_contracts::{CommandEnvelope, EventEnvelope};
use aria_storage_engine::Database;
use rusqlite::{params, Connection, Transaction};
use uuid::Uuid;

/// انواع موجودیت مجاز برای بازنویسی دستی.
pub const OVERRIDE_ENTITY_TYPES: [&str; 3] = ["journal_trade", "entry_leg", "exit_leg"];

/// انواع مجاز پیوند پیوست.
pub const LINK_KINDS: [&str; 5] = ["before_trade", "after_trade", "chart", "news", "other"];

/// فراداده بازنویسی خوانده‌شده از پایگاه‌داده: (reversible, entity_type, entity_id).
type OverrideMeta = (i64, String, String);

/// فراداده معامله برای بازمحاسبه R:
/// (direction, commission, swap, initial_stop_loss, manual_risk).
type TradeRiskMeta = (String, f64, f64, Option<f64>, Option<f64>);

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// سرویس دامنه معاملاتی روی یک پایگاه‌داده باز.
pub struct DomainService<'a> {
    db: &'a Database,
}

impl<'a> DomainService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    // ==================== اجرای دستور ====================

    /// اجرای پاکت دستور — اتمیک، با نوشتن رویدادها در outbox.
    /// رویدادهای بازگشتی فقط پس از commit موفق باید منتشر شوند.
    pub fn execute(&self, cmd: &CommandEnvelope) -> Result<Vec<EventEnvelope>, DomainError> {
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| DomainError::storage(e.to_string()))?;
        let events = self.dispatch(&tx, cmd)?;
        for ev in &events {
            write_event_tx(&tx, ev)?;
        }
        tx.commit().map_err(|e| DomainError::storage(e.to_string()))?;
        Ok(events)
    }

    fn dispatch(&self, tx: &Transaction<'_>, cmd: &CommandEnvelope) -> Result<Vec<EventEnvelope>, DomainError> {
        let factory = DomainEventFactory::new(cmd.correlation_id);
        match cmd.command_type.as_str() {
            command_type::CREATE_TRADE => {
                let c: CreateTradeCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let trade_id = self.create_trade(tx, &c)?;
                Ok(vec![
                    factory.trade_created(&trade_id),
                    factory.stats_invalidated(Some(&trade_id)),
                ])
            }
            command_type::UPDATE_TRADE => {
                let c: UpdateTradeCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let trade_id = self.update_trade(tx, &c)?;
                Ok(vec![
                    factory.trade_updated(&trade_id),
                    factory.stats_invalidated(Some(&trade_id)),
                ])
            }
            command_type::DELETE_TRADE => {
                let c: DeleteTradeCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                self.delete_trade(tx, &c)?;
                Ok(vec![
                    factory.trade_deleted(&c.trade_id),
                    factory.stats_invalidated(Some(&c.trade_id)),
                ])
            }
            command_type::ADD_ENTRY_LEG => {
                let c: AddEntryLegCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let leg_id = self.add_entry_leg(tx, &c)?;
                Ok(vec![
                    factory.entry_leg_added(&c.trade_id, &leg_id),
                    factory.stats_invalidated(Some(&c.trade_id)),
                ])
            }
            command_type::UPDATE_ENTRY_LEG => {
                let c: UpdateEntryLegCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let trade_id = self.update_entry_leg(tx, &c)?;
                Ok(vec![
                    factory.trade_updated(&trade_id),
                    factory.stats_invalidated(Some(&trade_id)),
                ])
            }
            command_type::ADD_EXIT_LEG => {
                let c: AddExitLegCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let leg_id = self.add_exit_leg(tx, &c)?;
                Ok(vec![
                    factory.exit_leg_added(&c.trade_id, &leg_id),
                    factory.stats_invalidated(Some(&c.trade_id)),
                ])
            }
            command_type::UPDATE_EXIT_LEG => {
                let c: UpdateExitLegCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let trade_id = self.update_exit_leg(tx, &c)?;
                Ok(vec![
                    factory.trade_updated(&trade_id),
                    factory.stats_invalidated(Some(&trade_id)),
                ])
            }
            command_type::ASSIGN_EXECUTION_TO_LEG => {
                let c: AssignExecutionToLegCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let trade_id = self.assign_execution_to_leg(tx, &c)?;
                Ok(vec![
                    factory.execution_assigned(&c.execution_id, &c.leg_id, &trade_id),
                    factory.stats_invalidated(Some(&trade_id)),
                ])
            }
            command_type::ADD_MANUAL_OVERRIDE => {
                let c: AddManualOverrideCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let id = self.add_manual_override(tx, &c)?;
                Ok(vec![
                    factory.override_added(&id, &c.entity_type, &c.entity_id, &c.field_name),
                    factory.stats_invalidated(
                        self.trade_id_of_entity(tx, &c.entity_type, &c.entity_id)?.as_deref(),
                    ),
                ])
            }
            command_type::REVERT_MANUAL_OVERRIDE => {
                let c: RevertManualOverrideCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                let (entity_type, entity_id) = self.revert_manual_override(tx, &c)?;
                Ok(vec![
                    factory.override_reverted(&c.override_id),
                    factory.stats_invalidated(
                        self.trade_id_of_entity(tx, &entity_type, &entity_id)?.as_deref(),
                    ),
                ])
            }
            command_type::LINK_ATTACHMENT_TO_TRADE => {
                let c: LinkAttachmentToTradeCommand = serde_json::from_value(cmd.payload.clone())
                    .map_err(|e| DomainError::invalid(format!("bad payload: {e}")))?;
                self.link_attachment_to_trade(tx, &c)?;
                Ok(vec![
                    factory.trade_updated(&c.trade_id),
                    factory.stats_invalidated(Some(&c.trade_id)),
                ])
            }
            other => Err(DomainError::invalid(format!("unknown command_type: {other}"))),
        }
    }

    // ==================== هندلرهای دستور ====================

    fn create_trade(&self, tx: &Transaction<'_>, c: &CreateTradeCommand) -> Result<String, DomainError> {
        let direction = Direction::parse(&c.direction)
            .ok_or_else(|| DomainError::invalid(format!("invalid direction: {}", c.direction)))?;
        if !self.exists(tx, "trading_accounts", &c.account_id)? {
            return Err(DomainError::account_not_found(&c.account_id));
        }
        if !self.exists(tx, "symbols", &c.symbol_id)? {
            return Err(DomainError::symbol_not_found(&c.symbol_id));
        }
        if c.commission < 0.0 || c.swap < 0.0 {
            return Err(DomainError::invalid("commission/swap must be non-negative"));
        }
        let id = Uuid::new_v4().to_string();
        let now = now_iso();
        tx.execute(
            "INSERT INTO journal_trades (
                id, account_id, symbol_id, direction, status, strategy, timeframe, session,
                market_condition, entry_type, note, tags, emotions, mistakes, entry_time,
                initial_stop_loss, take_profit, manual_risk, commission, swap,
                created_at, updated_at
             ) VALUES (?1,?2,?3,?4,'open',?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?20)",
            params![
                id, c.account_id, c.symbol_id, direction.as_str(), c.strategy, c.timeframe,
                c.session, c.market_condition, c.entry_type, c.note, c.tags, c.emotions,
                c.mistakes, c.entry_time, c.initial_stop_loss, c.take_profit, c.manual_risk,
                c.commission, c.swap, now
            ],
        )?;
        self.recompute_risk_tx(tx, &id)?;
        Ok(id)
    }

    fn update_trade(&self, tx: &Transaction<'_>, c: &UpdateTradeCommand) -> Result<String, DomainError> {
        self.require_live_trade(tx, &c.trade_id)?;
        if let Some(s) = &c.status {
            TradeStatus::parse(s).ok_or_else(|| DomainError::invalid(format!("invalid status: {s}")))?;
        }
        let mut clauses: Vec<String> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        let updates = [
            ("strategy", c.strategy.clone().map(rusqlite::types::Value::Text)),
            ("timeframe", c.timeframe.clone().map(rusqlite::types::Value::Text)),
            ("session", c.session.clone().map(rusqlite::types::Value::Text)),
            ("market_condition", c.market_condition.clone().map(rusqlite::types::Value::Text)),
            ("entry_type", c.entry_type.clone().map(rusqlite::types::Value::Text)),
            ("note", c.note.clone().map(rusqlite::types::Value::Text)),
            ("tags", c.tags.clone().map(rusqlite::types::Value::Text)),
            ("emotions", c.emotions.clone().map(rusqlite::types::Value::Text)),
            ("mistakes", c.mistakes.clone().map(rusqlite::types::Value::Text)),
            ("entry_time", c.entry_time.clone().map(rusqlite::types::Value::Text)),
            ("exit_time", c.exit_time.clone().map(rusqlite::types::Value::Text)),
            ("status", c.status.clone().map(rusqlite::types::Value::Text)),
            ("initial_stop_loss", c.initial_stop_loss.map(rusqlite::types::Value::Real)),
            ("take_profit", c.take_profit.map(rusqlite::types::Value::Real)),
            ("manual_risk", c.manual_risk.map(rusqlite::types::Value::Real)),
            ("commission", c.commission.map(rusqlite::types::Value::Real)),
            ("swap", c.swap.map(rusqlite::types::Value::Real)),
        ];
        for (col, v) in updates {
            if let Some(val) = v {
                clauses.push(format!("{col} = ?"));
                values.push(val);
            }
        }
        if clauses.is_empty() {
            return Err(DomainError::invalid("update_trade: no fields provided"));
        }
        values.push(rusqlite::types::Value::Text(now_iso()));
        values.push(rusqlite::types::Value::Text(c.trade_id.clone()));
        let sql = format!(
            "UPDATE journal_trades SET {}, updated_at = ? WHERE id = ?",
            clauses.join(", ")
        );
        tx.execute(&sql, rusqlite::params_from_iter(values.iter()))?;
        self.recompute_risk_tx(tx, &c.trade_id)?;
        Ok(c.trade_id.clone())
    }

    fn delete_trade(&self, tx: &Transaction<'_>, c: &DeleteTradeCommand) -> Result<(), DomainError> {
        self.require_live_trade(tx, &c.trade_id)?;
        let n = tx.execute(
            "UPDATE journal_trades SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now_iso(), c.trade_id],
        )?;
        if n == 0 {
            return Err(DomainError::trade_not_found(&c.trade_id));
        }
        // ثبت در حسابرسی — حذف دستوری باید ردیابی‌پذیر باشد
        tx.execute(
            "INSERT INTO audit_logs (id, action, actor, target, detail, created_at)
             VALUES (?1, 'trade.delete', 'kernel:domain_engine', ?2, ?3, ?4)",
            params![Uuid::new_v4().to_string(), c.trade_id, c.reason, now_iso()],
        )?;
        Ok(())
    }

    fn add_entry_leg(&self, tx: &Transaction<'_>, c: &AddEntryLegCommand) -> Result<String, DomainError> {
        self.require_live_trade(tx, &c.trade_id)?;
        if c.volume <= 0.0 || !c.volume.is_finite() {
            return Err(DomainError::invalid("volume must be positive and finite"));
        }
        let id = Uuid::new_v4().to_string();
        let now = now_iso();
        tx.execute(
            "INSERT INTO entry_legs (id, trade_id, planned_price, executed_price, volume,
             stop_loss, take_profit, entry_time, note, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            params![id, c.trade_id, c.planned_price, c.executed_price, c.volume,
                    c.stop_loss, c.take_profit, c.entry_time, c.note, now],
        )?;
        // قرارداد دامنه: معامله دستی → ساخت Execution دستی و اتصال خودش به پا
        if let Some(price) = c.executed_price {
            let direction: String = tx.query_row(
                "SELECT direction FROM journal_trades WHERE id = ?1",
                params![c.trade_id],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO executions (id, trade_id, leg_id, leg_kind, kind, direction,
                 price, volume, executed_at, assignment_status, created_at)
                 VALUES (?1,?2,?3,'entry','manual',?4,?5,?6,?7,'assigned',?8)",
                params![
                    Uuid::new_v4().to_string(),
                    c.trade_id,
                    id,
                    direction,
                    price,
                    c.volume,
                    c.entry_time.clone().unwrap_or_else(|| now.clone()),
                    now
                ],
            )?;
        }
        self.recompute_risk_tx(tx, &c.trade_id)?;
        Ok(id)
    }

    fn update_entry_leg(&self, tx: &Transaction<'_>, c: &UpdateEntryLegCommand) -> Result<String, DomainError> {
        let trade_id = self.leg_trade_id(tx, "entry_legs", &c.leg_id)?;
        // پاهای معامله حذف‌نرم‌شده تغییرناپذیرند
        self.require_live_trade(tx, &trade_id)?;
        if let Some(v) = c.volume {
            if v <= 0.0 || !v.is_finite() {
                return Err(DomainError::invalid("volume must be positive and finite"));
            }
        }
        let mut clauses: Vec<String> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        for (col, v) in [
            ("planned_price", c.planned_price.map(rusqlite::types::Value::Real)),
            ("executed_price", c.executed_price.map(rusqlite::types::Value::Real)),
            ("volume", c.volume.map(rusqlite::types::Value::Real)),
            ("stop_loss", c.stop_loss.map(rusqlite::types::Value::Real)),
            ("take_profit", c.take_profit.map(rusqlite::types::Value::Real)),
            ("entry_time", c.entry_time.clone().map(rusqlite::types::Value::Text)),
            ("note", c.note.clone().map(rusqlite::types::Value::Text)),
        ] {
            if let Some(val) = v {
                clauses.push(format!("{col} = ?"));
                values.push(val);
            }
        }
        if clauses.is_empty() {
            return Err(DomainError::invalid("update_entry_leg: no fields provided"));
        }
        values.push(rusqlite::types::Value::Text(now_iso()));
        values.push(rusqlite::types::Value::Text(c.leg_id.clone()));
        let sql = format!("UPDATE entry_legs SET {}, updated_at = ? WHERE id = ?", clauses.join(", "));
        tx.execute(&sql, rusqlite::params_from_iter(values.iter()))?;
        // هم‌گام‌سازی اجرای دستی پیوسته به پا (قیمت/حجم/زمان)
        for (col, v) in [
            ("price", c.executed_price.map(rusqlite::types::Value::Real)),
            ("volume", c.volume.map(rusqlite::types::Value::Real)),
            ("executed_at", c.entry_time.clone().map(rusqlite::types::Value::Text)),
        ] {
            if let Some(val) = v {
                tx.execute(
                    &format!(
                        "UPDATE executions SET {col} = ?1 WHERE leg_id = ?2 AND kind = 'manual'"
                    ),
                    rusqlite::params_from_iter([
                        val,
                        rusqlite::types::Value::Text(c.leg_id.clone()),
                    ]),
                )?;
            }
        }
        self.recompute_risk_tx(tx, &trade_id)?;
        Ok(trade_id)
    }

    fn add_exit_leg(&self, tx: &Transaction<'_>, c: &AddExitLegCommand) -> Result<String, DomainError> {
        self.require_live_trade(tx, &c.trade_id)?;
        if c.volume <= 0.0 || !c.volume.is_finite() {
            return Err(DomainError::invalid("volume must be positive and finite"));
        }
        let id = Uuid::new_v4().to_string();
        let now = now_iso();
        tx.execute(
            "INSERT INTO exit_legs (id, trade_id, exit_reason, executed_price, volume,
             exit_time, note, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
            params![id, c.trade_id, c.exit_reason, c.executed_price, c.volume,
                    c.exit_time, c.note, now],
        )?;
        // قرارداد دامنه: معامله دستی → ساخت Execution دستی و اتصال خودش به پا
        if let Some(price) = c.executed_price {
            let direction: String = tx.query_row(
                "SELECT direction FROM journal_trades WHERE id = ?1",
                params![c.trade_id],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO executions (id, trade_id, leg_id, leg_kind, kind, direction,
                 price, volume, executed_at, assignment_status, created_at)
                 VALUES (?1,?2,?3,'exit','manual',?4,?5,?6,?7,'assigned',?8)",
                params![
                    Uuid::new_v4().to_string(),
                    c.trade_id,
                    id,
                    direction,
                    price,
                    c.volume,
                    c.exit_time.clone().unwrap_or_else(|| now.clone()),
                    now
                ],
            )?;
        }
        self.recompute_risk_tx(tx, &c.trade_id)?;
        Ok(id)
    }

    fn update_exit_leg(&self, tx: &Transaction<'_>, c: &UpdateExitLegCommand) -> Result<String, DomainError> {
        let trade_id = self.leg_trade_id(tx, "exit_legs", &c.leg_id)?;
        // پاهای معامله حذف‌نرم‌شده تغییرناپذیرند
        self.require_live_trade(tx, &trade_id)?;
        if let Some(v) = c.volume {
            if v <= 0.0 || !v.is_finite() {
                return Err(DomainError::invalid("volume must be positive and finite"));
            }
        }
        let mut clauses: Vec<String> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        for (col, v) in [
            ("exit_reason", c.exit_reason.clone().map(rusqlite::types::Value::Text)),
            ("executed_price", c.executed_price.map(rusqlite::types::Value::Real)),
            ("volume", c.volume.map(rusqlite::types::Value::Real)),
            ("exit_time", c.exit_time.clone().map(rusqlite::types::Value::Text)),
            ("note", c.note.clone().map(rusqlite::types::Value::Text)),
        ] {
            if let Some(val) = v {
                clauses.push(format!("{col} = ?"));
                values.push(val);
            }
        }
        if clauses.is_empty() {
            return Err(DomainError::invalid("update_exit_leg: no fields provided"));
        }
        values.push(rusqlite::types::Value::Text(now_iso()));
        values.push(rusqlite::types::Value::Text(c.leg_id.clone()));
        let sql = format!("UPDATE exit_legs SET {}, updated_at = ? WHERE id = ?", clauses.join(", "));
        tx.execute(&sql, rusqlite::params_from_iter(values.iter()))?;
        // هم‌گام‌سازی اجرای دستی پیوسته به پا (قیمت/حجم/زمان)
        for (col, v) in [
            ("price", c.executed_price.map(rusqlite::types::Value::Real)),
            ("volume", c.volume.map(rusqlite::types::Value::Real)),
            ("executed_at", c.exit_time.clone().map(rusqlite::types::Value::Text)),
        ] {
            if let Some(val) = v {
                tx.execute(
                    &format!(
                        "UPDATE executions SET {col} = ?1 WHERE leg_id = ?2 AND kind = 'manual'"
                    ),
                    rusqlite::params_from_iter([
                        val,
                        rusqlite::types::Value::Text(c.leg_id.clone()),
                    ]),
                )?;
            }
        }
        self.recompute_risk_tx(tx, &trade_id)?;
        Ok(trade_id)
    }

    fn assign_execution_to_leg(&self, tx: &Transaction<'_>, c: &AssignExecutionToLegCommand) -> Result<String, DomainError> {
        // اجرا باید وجود داشته باشد
        let exec: Option<(Option<String>, String, String)> = tx
            .query_row(
                "SELECT trade_id, assignment_status, direction FROM executions WHERE id = ?1",
                params![c.execution_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let (exec_trade, _status, exec_direction) =
            exec.ok_or_else(|| DomainError::execution_not_found(&c.execution_id))?;

        // نوع پا را با جست‌وجوی هر دو جدول می‌یابیم
        let leg_kind = if self.exists(tx, "entry_legs", &c.leg_id)? {
            "entry"
        } else if self.exists(tx, "exit_legs", &c.leg_id)? {
            "exit"
        } else {
            return Err(DomainError::leg_not_found(&c.leg_id));
        };
        let leg_trade: String = tx.query_row(
            &format!("SELECT trade_id FROM {leg_kind}_legs WHERE id = ?1"),
            params![c.leg_id],
            |r| r.get(0),
        )?;

        // اجرا نباید به معامله دیگری تعلق داشته باشد
        if let Some(t) = &exec_trade {
            if t != &leg_trade {
                return Err(DomainError::assignment_conflict(format!(
                    "execution belongs to trade {t}, leg belongs to trade {leg_trade}"
                )));
            }
        }

        // راست‌آزمایی جهت — اجرای «assigned» باید هم‌جهت با معامله باشد؛
        // اجرای «needs_assignment» وارداتی جهت موقتی دارد و جهت قطعی معامله
        // معیار است (نرمال‌سازی در لحظه تخصیص).
        let trade_direction: String = tx.query_row(
            "SELECT direction FROM journal_trades WHERE id = ?1",
            params![leg_trade],
            |r| r.get(0),
        )?;
        if exec_direction != trade_direction {
            if _status != "needs_assignment" {
                return Err(DomainError::assignment_conflict(format!(
                    "execution direction {exec_direction} does not match trade direction {trade_direction}"
                )));
            }
            tx.execute(
                "UPDATE executions SET direction = ?1 WHERE id = ?2",
                params![trade_direction, c.execution_id],
            )?;
        }

        tx.execute(
            "UPDATE executions SET trade_id = ?1, leg_id = ?2, leg_kind = ?3,
             assignment_status = 'assigned' WHERE id = ?4",
            params![leg_trade, c.leg_id, leg_kind, c.execution_id],
        )?;
        // حسابرسی تخصیص — تصمیم مؤثر بر PnL باید ردیابی‌پذیر باشد
        tx.execute(
            "INSERT INTO audit_logs (id, action, actor, target, detail, created_at)
             VALUES (?1, 'execution.assign', 'kernel:domain_engine', ?2, ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                leg_trade,
                format!("execution {} assigned to {} leg {}", c.execution_id, leg_kind, c.leg_id),
                now_iso()
            ],
        )?;
        self.recompute_risk_tx(tx, &leg_trade)?;
        Ok(leg_trade)
    }

    fn add_manual_override(&self, tx: &Transaction<'_>, c: &AddManualOverrideCommand) -> Result<String, DomainError> {
        if !OVERRIDE_ENTITY_TYPES.contains(&c.entity_type.as_str()) {
            return Err(DomainError::invalid(format!("invalid entity_type: {}", c.entity_type)));
        }
        if c.field_name.trim().is_empty() {
            return Err(DomainError::invalid("field_name must not be empty"));
        }
        if !self.exists(tx, table_of(&c.entity_type), &c.entity_id)? {
            return Err(match c.entity_type.as_str() {
                "journal_trade" => DomainError::trade_not_found(&c.entity_id),
                _ => DomainError::leg_not_found(&c.entity_id),
            });
        }
        // موجودیت باید زنده باشد — روی معامله حذف‌نرم‌شده بازنویسی مجاز نیست
        if let Some(tid) = self.trade_id_of_entity(tx, &c.entity_type, &c.entity_id)? {
            self.require_live_trade(tx, &tid)?;
        }
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO manual_overrides (id, entity_type, entity_id, field_name,
             previous_value, new_value, reason, source, priority, reversible,
             created_by, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![id, c.entity_type, c.entity_id, c.field_name, c.previous_value,
                    c.new_value, c.reason, c.source, c.priority, c.reversible as i64,
                    c.created_by, now_iso()],
        )?;
        Ok(id)
    }

    fn revert_manual_override(&self, tx: &Transaction<'_>, c: &RevertManualOverrideCommand) -> Result<(String, String), DomainError> {
        let row: Option<OverrideMeta> = tx
            .query_row(
                "SELECT reversible, entity_type, entity_id FROM manual_overrides WHERE id = ?1",
                params![c.override_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let (reversible, entity_type, entity_id) =
            row.ok_or_else(|| DomainError::override_error(format!("override not found: {}", c.override_id)))?;
        if reversible == 0 {
            return Err(DomainError::override_error("override is not reversible"));
        }
        let n = tx.execute(
            "UPDATE manual_overrides SET reverted_at = ?1 WHERE id = ?2 AND reverted_at IS NULL",
            params![now_iso(), c.override_id],
        )?;
        if n == 0 {
            return Err(DomainError::override_error("override already reverted"));
        }
        Ok((entity_type, entity_id))
    }

    fn link_attachment_to_trade(&self, tx: &Transaction<'_>, c: &LinkAttachmentToTradeCommand) -> Result<(), DomainError> {
        if !LINK_KINDS.contains(&c.link_kind.as_str()) {
            return Err(DomainError::invalid(format!("invalid link_kind: {}", c.link_kind)));
        }
        if !self.exists(tx, "attachments", &c.attachment_id)? {
            return Err(DomainError::attachment_not_found(&c.attachment_id));
        }
        self.require_live_trade(tx, &c.trade_id)?;
        tx.execute(
            "INSERT INTO attachment_trade_links (id, attachment_id, trade_id, link_kind, created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![Uuid::new_v4().to_string(), c.attachment_id, c.trade_id, c.link_kind, now_iso()],
        )?;
        Ok(())
    }

    // ==================== بازمحاسبه R ====================

    /// بازمحاسبه و پاید‌سازی ستون‌های R معامله در همان تراکنش.
    pub(crate) fn recompute_risk_tx(&self, tx: &Transaction<'_>, trade_id: &str) -> Result<(), DomainError> {
        let trade: Option<TradeRiskMeta> = tx
            .query_row(
                "SELECT direction, commission, swap, initial_stop_loss, manual_risk
                 FROM journal_trades WHERE id = ?1",
                params![trade_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let Some((direction_text, commission, swap, initial_sl, manual_risk)) = trade else {
            return Err(DomainError::trade_not_found(trade_id));
        };
        let direction = parse_direction(&direction_text)?;

        let (take_profit, contract_size): (Option<f64>, f64) = tx.query_row(
            "SELECT j.take_profit, s.contract_size FROM journal_trades j
             JOIN symbols s ON s.id = j.symbol_id WHERE j.id = ?1",
            params![trade_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        let mut entry_stmt = tx.prepare(
            "SELECT executed_price, volume FROM entry_legs
             WHERE trade_id = ?1 AND executed_price IS NOT NULL",
        )?;
        let entry_legs: Vec<EntryLegInput> = entry_stmt
            .query_map(params![trade_id], |r| {
                Ok(EntryLegInput { executed_price: r.get(0)?, volume: r.get(1)? })
            })?
            .collect::<Result<_, _>>()?;
        drop(entry_stmt);

        let mut exit_stmt = tx.prepare(
            "SELECT executed_price, volume FROM exit_legs
             WHERE trade_id = ?1 AND executed_price IS NOT NULL",
        )?;
        let exit_legs: Vec<ExitLegInput> = exit_stmt
            .query_map(params![trade_id], |r| {
                Ok(ExitLegInput { executed_price: r.get(0)?, volume: r.get(1)? })
            })?
            .collect::<Result<_, _>>()?;
        drop(exit_stmt);

        let has_unassigned: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM executions WHERE trade_id = ?1 AND assignment_status = 'needs_assignment')",
            params![trade_id],
            |r| r.get::<_, i64>(0).map(|v| v != 0),
        )?;

        // قیمت ورود برنامه‌ریزی‌شده برای PlannedR (میانگین وزنی planned_price ها)
        let mut planned_stmt = tx.prepare(
            "SELECT planned_price, volume FROM entry_legs
             WHERE trade_id = ?1 AND planned_price IS NOT NULL",
        )?;
        let planned: Vec<(f64, f64)> = planned_stmt
            .query_map(params![trade_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        drop(planned_stmt);
        let planned_entry = if planned.is_empty() {
            None
        } else {
            let (sum, vol): (f64, f64) =
                planned.iter().fold((0.0, 0.0), |(s, v), (p, q)| (s + p * q, v + q));
            if vol > 0.0 {
                Some(sum / vol)
            } else {
                None
            }
        };

        let comp = compute_trade_risk(
            direction.pnl_sign(),
            contract_size,
            commission,
            swap,
            initial_sl,
            manual_risk,
            take_profit,
            planned_entry,
            &entry_legs,
            &exit_legs,
            has_unassigned,
        );

        tx.execute(
            "UPDATE journal_trades SET risk_calculation_status = ?1, risk_basis = ?2,
             planned_r = ?3, initial_risk_amount = ?4, realized_pnl = ?5,
             realized_r = ?6, trade_r = ?7, updated_at = ?8 WHERE id = ?9",
            params![
                comp.status.as_str(),
                comp.basis.map(|b| b.as_str()),
                comp.planned_r,
                comp.initial_risk_amount,
                comp.realized_pnl,
                comp.realized_r,
                comp.trade_r,
                now_iso(),
                trade_id
            ],
        )?;
        Ok(())
    }

    /// بازمحاسبه R یک معامله روی اتصال مستقل — برای جریان ایمپورت (فاز ۱.۱۴)
    /// و هر جایی که اجراهای تخصیص‌نیافته پس از ثبت داده خام باید آمار را به‌روز کنند.
    pub fn recompute_risk(&self, trade_id: &str) -> Result<(), DomainError> {
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| DomainError::storage(e.to_string()))?;
        self.recompute_risk_tx(&tx, trade_id)?;
        tx.commit().map_err(|e| DomainError::storage(e.to_string()))?;
        Ok(())
    }

    // ==================== خواندن ====================

    /// خواندن یک معامله زنده (حذف‌نرم‌شده).
    pub fn get_trade(&self, trade_id: &str) -> Result<Option<Trade>, DomainError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, account_id, symbol_id, direction, status, strategy, timeframe, session,
             market_condition, entry_type, note, tags, emotions, mistakes, entry_time, exit_time,
             initial_stop_loss, take_profit, manual_risk, risk_calculation_status, risk_basis,
             planned_r, initial_risk_amount, realized_pnl, realized_r, trade_r, commission, swap,
             position_group_id, deleted_at, created_at, updated_at
             FROM journal_trades WHERE id = ?1 AND deleted_at IS NULL",
        )?;
        let mut rows = stmt.query(params![trade_id])?;
        match rows.next()? {
            Some(r) => Ok(Some(map_trade_row(r)?)),
            None => Ok(None),
        }
    }

    /// فهرست معامله‌های یک حساب (به ترتیب زمان ورود).
    pub fn list_trades(&self, account_id: &str, include_deleted: bool) -> Result<Vec<Trade>, DomainError> {
        let conn = self.db.lock();
        let filter = if include_deleted { "" } else { " AND deleted_at IS NULL" };
        let sql = format!(
            "SELECT id, account_id, symbol_id, direction, status, strategy, timeframe, session,
             market_condition, entry_type, note, tags, emotions, mistakes, entry_time, exit_time,
             initial_stop_loss, take_profit, manual_risk, risk_calculation_status, risk_basis,
             planned_r, initial_risk_amount, realized_pnl, realized_r, trade_r, commission, swap,
             position_group_id, deleted_at, created_at, updated_at
             FROM journal_trades WHERE account_id = ?1{filter} ORDER BY entry_time, created_at"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![account_id], map_trade_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// پاهای ورود یک معامله.
    pub fn entry_legs(&self, trade_id: &str) -> Result<Vec<EntryLeg>, DomainError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, trade_id, planned_price, executed_price, volume, stop_loss,
             take_profit, entry_time, note, created_at, updated_at
             FROM entry_legs WHERE trade_id = ?1
               AND EXISTS (SELECT 1 FROM journal_trades t
                           WHERE t.id = entry_legs.trade_id AND t.deleted_at IS NULL)
             ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![trade_id], |r| {
            Ok(EntryLeg {
                id: r.get(0)?,
                trade_id: r.get(1)?,
                planned_price: r.get(2)?,
                executed_price: r.get(3)?,
                volume: r.get(4)?,
                stop_loss: r.get(5)?,
                take_profit: r.get(6)?,
                entry_time: r.get(7)?,
                note: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// پاهای خروج یک معامله.
    pub fn exit_legs(&self, trade_id: &str) -> Result<Vec<ExitLeg>, DomainError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, trade_id, exit_reason, executed_price, volume, exit_time, note,
             created_at, updated_at FROM exit_legs WHERE trade_id = ?1
               AND EXISTS (SELECT 1 FROM journal_trades t
                           WHERE t.id = exit_legs.trade_id AND t.deleted_at IS NULL)
             ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![trade_id], |r| {
            Ok(ExitLeg {
                id: r.get(0)?,
                trade_id: r.get(1)?,
                exit_reason: r.get(2)?,
                executed_price: r.get(3)?,
                volume: r.get(4)?,
                exit_time: r.get(5)?,
                note: r.get(6)?,
                created_at: r.get(7)?,
                updated_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// اجراهای یک معامله.
    pub fn executions(&self, trade_id: &str) -> Result<Vec<Execution>, DomainError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, trade_id, leg_id, leg_kind, source_record_id, kind, direction, price,
             volume, executed_at, commission, swap, ticket, magic, comment, assignment_status,
             created_at FROM executions WHERE trade_id = ?1
               AND EXISTS (SELECT 1 FROM journal_trades t
                           WHERE t.id = executions.trade_id AND t.deleted_at IS NULL)
             ORDER BY executed_at",
        )?;
        let rows = stmt.query_map(params![trade_id], map_execution_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// بازنویسی‌های یک موجودیت.
    pub fn overrides_of(&self, entity_type: &str, entity_id: &str, include_reverted: bool) -> Result<Vec<ManualOverride>, DomainError> {
        let conn = self.db.lock();
        // موجودیت متعلق به معامله حذف‌نرم‌شده → بازنویسی‌هایش هم پنهان است
        if entity_hidden_by_soft_delete(&conn, entity_type, entity_id)? {
            return Ok(vec![]);
        }
        let filter = if include_reverted { "" } else { " AND reverted_at IS NULL" };
        let sql = format!(
            "SELECT id, entity_type, entity_id, field_name, previous_value, new_value, reason,
             source, priority, reversible, created_by, created_at, reverted_at
             FROM manual_overrides WHERE entity_type = ?1 AND entity_id = ?2{filter}
             ORDER BY priority DESC, created_at ASC"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![entity_type, entity_id], map_override_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// داده مؤثر = canonical + بازنویسی‌های فعال (به ترتیب اولویت).
    /// خروجی JSON — منبع نهایی نمایش/فیلتر/آمار طبق قرارداد دامنه.
    pub fn effective_entity(&self, entity_type: &str, entity_id: &str) -> Result<Option<serde_json::Value>, DomainError> {
        if !OVERRIDE_ENTITY_TYPES.contains(&entity_type) {
            return Err(DomainError::invalid(format!("invalid entity_type: {entity_type}")));
        }
        let table = table_of(entity_type);
        let conn = self.db.lock();
        // موجودیت متعلق به معامله حذف‌نرم‌شده → مانند یافت‌نشدن
        if entity_hidden_by_soft_delete(&conn, entity_type, entity_id)? {
            return Ok(None);
        }
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table} WHERE id = ?1"))?;
        // نام ستون‌ها قبل از ساخت query خوانده می‌شود (stmt موقتاً به rows قرض داده می‌شود)
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt.query(params![entity_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let mut obj = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            let v: rusqlite::types::Value = row.get(i)?;
            obj.insert(name.clone(), sql_value_to_json(&v));
        }
        drop(rows);
        drop(stmt);

        // اعمال بازنویسی‌های فعال — مستقیم روی همین اتصال (بدون قفل دوباره)
        let mut ov_stmt = conn.prepare(
            "SELECT id, entity_type, entity_id, field_name, previous_value, new_value, reason,
             source, priority, reversible, created_by, created_at, reverted_at
             FROM manual_overrides WHERE entity_type = ?1 AND entity_id = ?2 AND reverted_at IS NULL
             ORDER BY priority DESC, created_at ASC",
        )?;
        let ov_rows = ov_stmt.query_map(params![entity_type, entity_id], map_override_row)?;
        for ov in ov_rows {
            let ov = ov?;
            let parsed = match ov.new_value.as_deref() {
                Some(v) => serde_json::from_str::<serde_json::Value>(v)
                    .unwrap_or_else(|_| serde_json::Value::String(v.to_string())),
                None => serde_json::Value::Null,
            };
            obj.insert(ov.field_name.clone(), parsed);
        }
        Ok(Some(serde_json::Value::Object(obj)))
    }

    // ==================== outbox ====================

    /// رویدادهای منتشرنشده outbox (برای انتشار پس از commit).
    pub fn unpublished_events(&self, limit: usize) -> Result<Vec<EventEnvelope>, DomainError> {
        let conn = self.db.lock();
        // ترتیب قطعی: زمان ایجاد سپس ترتیب درج (rowid)
        let mut stmt = conn.prepare_cached(
            "SELECT id, event_type, event_version, source, correlation_id, payload, created_at
             FROM system_events WHERE published = 0 ORDER BY created_at, rowid LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, event_type, version, source, correlation, payload, created_at) = row?;
            let event_id = Uuid::parse_str(&id)
                .map_err(|e| DomainError::storage(format!("bad event_id in outbox: {e}")))?;
            let correlation_id = Uuid::parse_str(&correlation)
                .map_err(|e| DomainError::storage(format!("bad correlation_id: {e}")))?;
            let payload = serde_json::from_str(&payload)
                .map_err(|e| DomainError::storage(format!("bad event payload: {e}")))?;
            let source = match source.as_str() {
                "domain_engine" => aria_contracts::EventSource::DomainEngine,
                other => aria_contracts::EventSource::Other(other.to_string()),
            };
            // زمان رویداد از خود ردیف outbox می‌آید، نه از لحاظ خواندن
            let timestamp = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            out.push(EventEnvelope {
                event_id,
                event_type,
                event_version: version as u32,
                source,
                timestamp,
                correlation_id,
                payload,
            });
        }
        Ok(out)
    }

    /// علامت‌گذاری انتشار رویدادها (پس از broadcast موفق).
    /// فقط ردیف‌های هنوز منتشرنشده شمرده می‌شوند (idempotent — فراخوانی دوباره صفر می‌دهد).
    pub fn mark_events_published(&self, event_ids: &[Uuid]) -> Result<usize, DomainError> {
        if event_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.db.lock();
        let tx = conn.transaction().map_err(|e| DomainError::storage(e.to_string()))?;
        let mut n = 0;
        for id in event_ids {
            n += tx.execute(
                "UPDATE system_events SET published = 1 WHERE id = ?1 AND published = 0",
                params![id.to_string()],
            )?;
        }
        tx.commit().map_err(|e| DomainError::storage(e.to_string()))?;
        Ok(n)
    }

    // ==================== ابزارهای داخلی ====================

    fn exists(&self, tx: &Transaction<'_>, table: &str, id: &str) -> Result<bool, DomainError> {
        let found: i64 = tx.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            params![id],
            |r| r.get(0),
        )?;
        Ok(found != 0)
    }

    fn require_live_trade(&self, tx: &Transaction<'_>, trade_id: &str) -> Result<(), DomainError> {
        let deleted: Option<Option<String>> = tx
            .query_row(
                "SELECT deleted_at FROM journal_trades WHERE id = ?1",
                params![trade_id],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        match deleted {
            None => Err(DomainError::trade_not_found(trade_id)),
            Some(d) if d.is_some() => Err(DomainError::trade_not_found(trade_id)),
            Some(_) => Ok(()),
        }
    }

    fn leg_trade_id(&self, tx: &Transaction<'_>, table: &str, leg_id: &str) -> Result<String, DomainError> {
        tx.query_row(
            &format!("SELECT trade_id FROM {table} WHERE id = ?1"),
            params![leg_id],
            |r| r.get(0),
        )
        .map_err(|_| DomainError::leg_not_found(leg_id))
    }

    fn trade_id_of_entity(&self, tx: &Transaction<'_>, entity_type: &str, entity_id: &str) -> Result<Option<String>, DomainError> {
        match entity_type {
            "journal_trade" => Ok(Some(entity_id.to_string())),
            "entry_leg" | "exit_leg" => {
                let table = table_of(entity_type);
                Ok(tx
                    .query_row(
                        &format!("SELECT trade_id FROM {table} WHERE id = ?1"),
                        params![entity_id],
                        |r| r.get::<_, String>(0),
                    )
                    .map(Some)
                    .or_else(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => Ok(None),
                        other => Err(other),
                    })?)
            }
            other => Err(DomainError::invalid(format!("invalid entity_type: {other}"))),
        }
    }
}

/// نگاشت entity_type به جدول فیزیکی.
fn table_of(entity_type: &str) -> &'static str {
    match entity_type {
        "journal_trade" => "journal_trades",
        "entry_leg" => "entry_legs",
        "exit_leg" => "exit_legs",
        _ => "journal_trades",
    }
}

/// آیا موجودیت به معامله‌ای تعلق دارد که حذف نرم شده است؟
/// برای موجودیت‌های ناشناخته false برمی‌گرداند (رفتار پیشین حفظ می‌شود).
fn entity_hidden_by_soft_delete(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
) -> Result<bool, DomainError> {
    let related_trade: Option<String> = match entity_type {
        "journal_trade" => Some(entity_id.to_string()),
        "entry_leg" | "exit_leg" => conn
            .query_row(
                &format!("SELECT trade_id FROM {} WHERE id = ?1", table_of(entity_type)),
                params![entity_id],
                |r| r.get(0),
            )
            .map(Some)
            .unwrap_or(None),
        _ => None,
    };
    let Some(trade_id) = related_trade else {
        return Ok(false);
    };
    let deleted: Option<String> = conn
        .query_row(
            "SELECT deleted_at FROM journal_trades WHERE id = ?1",
            params![trade_id],
            |r| r.get(0),
        )
        .unwrap_or(None);
    Ok(deleted.is_some())
}

/// نوشتن رویداد در outbox داخل تراکنش جاری.
fn write_event_tx(tx: &Transaction<'_>, ev: &EventEnvelope) -> Result<(), DomainError> {
    tx.execute(
        "INSERT INTO system_events (id, event_type, event_version, source, correlation_id,
         payload, published, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,0,?7)",
        params![
            ev.event_id.to_string(),
            ev.event_type,
            ev.event_version as i64,
            ev.source.as_str(),
            ev.correlation_id.to_string(),
            ev.payload.to_string(),
            now_iso()
        ],
    )?;
    Ok(())
}

fn sql_value_to_json(v: &rusqlite::types::Value) -> serde_json::Value {
    match v {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::json!(i),
        rusqlite::types::Value::Real(f) => serde_json::json!(f),
        rusqlite::types::Value::Text(s) => serde_json::json!(s),
        rusqlite::types::Value::Blob(b) => serde_json::json!(b.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")),
    }
}

fn map_trade_row(r: &rusqlite::Row<'_>) -> Result<Trade, rusqlite::Error> {
    let direction_text: String = r.get(3)?;
    let status_text: String = r.get(4)?;
    Ok(Trade {
        id: r.get(0)?,
        account_id: r.get(1)?,
        symbol_id: r.get(2)?,
        direction: Direction::parse(&direction_text)
            .ok_or(rusqlite::Error::InvalidColumnType(3, "direction".into(), rusqlite::types::Type::Text))?,
        status: TradeStatus::parse(&status_text)
            .ok_or(rusqlite::Error::InvalidColumnType(4, "status".into(), rusqlite::types::Type::Text))?,
        strategy: r.get(5)?,
        timeframe: r.get(6)?,
        session: r.get(7)?,
        market_condition: r.get(8)?,
        entry_type: r.get(9)?,
        note: r.get(10)?,
        tags: r.get(11)?,
        emotions: r.get(12)?,
        mistakes: r.get(13)?,
        entry_time: r.get(14)?,
        exit_time: r.get(15)?,
        initial_stop_loss: r.get(16)?,
        take_profit: r.get(17)?,
        manual_risk: r.get(18)?,
        risk_calculation_status: r.get(19)?,
        risk_basis: r.get(20)?,
        planned_r: r.get(21)?,
        initial_risk_amount: r.get(22)?,
        realized_pnl: r.get(23)?,
        realized_r: r.get(24)?,
        trade_r: r.get(25)?,
        commission: r.get(26)?,
        swap: r.get(27)?,
        position_group_id: r.get(28)?,
        deleted_at: r.get(29)?,
        created_at: r.get(30)?,
        updated_at: r.get(31)?,
    })
}

fn map_execution_row(r: &rusqlite::Row<'_>) -> Result<Execution, rusqlite::Error> {
    let direction_text: String = r.get(6)?;
    let leg_kind: Option<String> = r.get(3)?;
    Ok(Execution {
        id: r.get(0)?,
        trade_id: r.get(1)?,
        leg_id: r.get(2)?,
        leg_kind: leg_kind.map(|k| match k.as_str() {
            "entry" => crate::model::LegKind::Entry,
            _ => crate::model::LegKind::Exit,
        }),
        source_record_id: r.get(4)?,
        kind: r.get(5)?,
        direction: Direction::parse(&direction_text)
            .ok_or(rusqlite::Error::InvalidColumnType(6, "direction".into(), rusqlite::types::Type::Text))?,
        price: r.get(7)?,
        volume: r.get(8)?,
        executed_at: r.get(9)?,
        commission: r.get(10)?,
        swap: r.get(11)?,
        ticket: r.get(12)?,
        magic: r.get(13)?,
        comment: r.get(14)?,
        assignment_status: r.get(15)?,
        created_at: r.get(16)?,
    })
}

fn map_override_row(r: &rusqlite::Row<'_>) -> Result<ManualOverride, rusqlite::Error> {
    Ok(ManualOverride {
        id: r.get(0)?,
        entity_type: r.get(1)?,
        entity_id: r.get(2)?,
        field_name: r.get(3)?,
        previous_value: r.get(4)?,
        new_value: r.get(5)?,
        reason: r.get(6)?,
        source: r.get(7)?,
        priority: r.get(8)?,
        reversible: r.get::<_, i64>(9)? != 0,
        created_by: r.get(10)?,
        created_at: r.get(11)?,
        reverted_at: r.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LegKind;

    fn db() -> Database {
        let db = Database::open_memory(Some("test-pass-1")).unwrap();
        {
            let mut conn = db.lock();
            aria_storage_engine::migrations::run_migrations(&mut conn, |_| Ok(()))
                .unwrap();
        }
        db
    }

    fn svc(db: &Database) -> DomainService<'_> {
        DomainService::new(db)
    }

    fn env(command_type: &str, payload: impl serde::Serialize) -> CommandEnvelope {
        CommandEnvelope::new(command_type, serde_json::to_value(payload).unwrap(), "ui")
    }

    /// ساخت حساب و نماد آزمایشی؛ خروجی (account_id, symbol_id).
    fn setup_account_symbol(db: &Database) -> (String, String) {
        let conn = db.lock();
        let now = "2026-09-28T12:00:00Z";
        conn.execute(
            "INSERT INTO profiles (id, name, created_at, updated_at) VALUES ('p1','پروفایل',?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO trading_accounts (id, profile_id, name, currency, created_at, updated_at)
             VALUES ('a1','p1','حساب','USD',?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols (id, name, contract_size, created_at) VALUES ('s1','XAUUSD',100.0,?1)",
            params![now],
        )
        .unwrap();
        ("a1".into(), "s1".into())
    }

    fn create_cmd(account: &str, symbol: &str, sl: Option<f64>) -> CreateTradeCommand {
        CreateTradeCommand {
            account_id: account.into(),
            symbol_id: symbol.into(),
            direction: "buy".into(),
            strategy: Some("breakout".into()),
            timeframe: None,
            session: None,
            market_condition: None,
            entry_type: None,
            note: None,
            tags: None,
            emotions: None,
            mistakes: None,
            entry_time: Some("2026-09-28T08:30:00Z".into()),
            initial_stop_loss: sl,
            take_profit: None,
            manual_risk: None,
            commission: 0.0,
            swap: 0.0,
        }
    }

    fn create_trade(db: &Database, a: &str, s: &str, sl: Option<f64>) -> String {
        let service = svc(db);
        let events = service
            .execute(&env(command_type::CREATE_TRADE, create_cmd(a, s, sl)))
            .unwrap();
        events[0].payload["trade_id"].as_str().unwrap().to_string()
    }

    fn add_entry(db: &Database, trade_id: &str, price: f64, vol: f64) -> String {
        let service = svc(db);
        let events = service
            .execute(&env(command_type::ADD_ENTRY_LEG, AddEntryLegCommand {
                trade_id: trade_id.to_string(),
                planned_price: None,
                executed_price: Some(price),
                volume: vol,
                stop_loss: None,
                take_profit: None,
                entry_time: None,
                note: None,
            }))
            .unwrap();
        events[0].payload["leg_id"].as_str().unwrap().to_string()
    }

    fn add_exit(db: &Database, trade_id: &str, price: f64, vol: f64) -> String {
        let service = svc(db);
        let events = service
            .execute(&env(command_type::ADD_EXIT_LEG, AddExitLegCommand {
                trade_id: trade_id.to_string(),
                exit_reason: None,
                executed_price: Some(price),
                volume: vol,
                exit_time: None,
                note: None,
            }))
            .unwrap();
        events[0].payload["leg_id"].as_str().unwrap().to_string()
    }

    fn trade_risk(db: &Database, trade_id: &str) -> (String, Option<f64>, Option<f64>) {
        let conn = db.lock();
        conn.query_row(
            "SELECT risk_calculation_status, realized_pnl, trade_r FROM journal_trades WHERE id = ?1",
            params![trade_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    }

    // ==================== دستورها و رویدادها ====================

    #[test]
    fn create_trade_persists_and_writes_outbox_events() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let service = svc(&db);
        let cmd = env(command_type::CREATE_TRADE, create_cmd(&a, &s, Some(90.0)));
        let events = service.execute(&cmd).unwrap();

        let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
        assert_eq!(types, vec!["domain.trade_created", "domain.stats_invalidated"]);
        // رویدادها با همان correlation_id دستور صادر شده‌اند
        assert!(events.iter().all(|e| e.correlation_id == cmd.correlation_id));
        // outbox داخل همان تراکنش نوشته شده (اتمیک)
        assert_eq!(service.unpublished_events(10).unwrap().len(), 2);
        let trade_id = events[0].payload["trade_id"].as_str().unwrap().to_string();
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        assert_eq!(trade.direction, Direction::Buy);
        assert_eq!(trade.status, TradeStatus::Open);
        // SL دارد ولی پای ورود ندارد → مبنا محاسبه‌شده، مبلغ هنوز None
        assert_eq!(trade.risk_calculation_status, "calculated");
        assert_eq!(trade.initial_risk_amount, None);
    }

    #[test]
    fn create_trade_rejects_unknown_account() {
        let db = db();
        let (_, s) = setup_account_symbol(&db);
        let service = svc(&db);
        let err = service
            .execute(&env(command_type::CREATE_TRADE, create_cmd("nope", &s, None)))
            .unwrap_err();
        assert_eq!(err.code(), 1402);
        // اتمیک: هیچ رویدادی نوشته نشده
        assert_eq!(service.unpublished_events(10).unwrap().len(), 0);
    }

    #[test]
    fn create_trade_rejects_bad_direction() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let service = svc(&db);
        let mut c = create_cmd(&a, &s, None);
        c.direction = "up".into();
        let err = service
            .execute(&env(command_type::CREATE_TRADE, c))
            .unwrap_err();
        assert_eq!(err.code(), 1404);
    }

    #[test]
    fn unknown_command_type_rejected() {
        let db = db();
        let service = svc(&db);
        let err = service
            .execute(&env("domain.unknown", serde_json::json!({})))
            .unwrap_err();
        assert_eq!(err.code(), 1404);
    }

    // ==================== پاها و محاسبه R پایان‌به‌پا ====================

    #[test]
    fn single_entry_single_exit_recomputes_r_end_to_end() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 115.0, 1.0);

        let (status, pnl, r) = trade_risk(&db, &trade_id);
        assert_eq!(status, "calculated");
        // ریسک = ۱۰ × ۱ × ۱۰۰ = ۱۰۰۰؛ سود = ۱۵ × ۱ × ۱۰۰ = ۱۵۰۰ → R = ۱.۵
        assert_eq!(pnl, Some(1500.0));
        assert_eq!(r, Some(1.5));

        let service = svc(&db);
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        assert_eq!(trade.risk_basis.as_deref(), Some("initial_stop_loss"));
        assert_eq!(trade.initial_risk_amount, Some(1000.0));
        assert_eq!(trade.realized_r, Some(1.5));
        assert_eq!(trade.planned_r, None);
    }

    #[test]
    fn multi_entry_weighted_average_persisted() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(102.5));
        add_entry(&db, &trade_id, 100.0, 1.0);
        add_entry(&db, &trade_id, 110.0, 3.0);
        add_exit(&db, &trade_id, 112.5, 4.0);

        let service = svc(&db);
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        // میانگین = ۱۰۷.۵؛ ریسک = ۵ × ۴ × ۱۰۰ = ۲۰۰۰؛ سود = ۵ × ۴ × ۱۰۰ = ۲۰۰۰ → R = ۱
        assert_eq!(trade.initial_risk_amount, Some(2000.0));
        assert_eq!(trade.realized_pnl, Some(2000.0));
        assert_eq!(trade.trade_r, Some(1.0));
    }

    #[test]
    fn multi_exit_summed_then_divided() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 110.0, 0.5);
        add_exit(&db, &trade_id, 120.0, 0.5);

        let service = svc(&db);
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        // سود = ۱۰×۰.۵ + ۲۰×۰.۵ = ۱۵ → R = ۱۵۰۰/۱۰۰۰ = ۱.۵
        assert_eq!(trade.realized_pnl, Some(1500.0));
        assert_eq!(trade.trade_r, Some(1.5));
    }

    #[test]
    fn commission_and_swap_persisted_end_to_end() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let service = svc(&db);
        let mut c = create_cmd(&a, &s, Some(90.0));
        c.commission = 200.0;
        c.swap = 100.0;
        let trade_id = service
            .execute(&env(command_type::CREATE_TRADE, c))
            .unwrap()[0]
            .payload["trade_id"]
            .as_str()
            .unwrap()
            .to_string();
        add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 115.0, 1.0);
        let (_, pnl, _) = trade_risk(&db, &trade_id);
        assert_eq!(pnl, Some(1200.0)); // ۱۵۰۰ − ۳۰۰
    }

    #[test]
    fn no_stop_loss_status_persisted() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        add_entry(&db, &trade_id, 100.0, 1.0);
        let (status, _, _) = trade_risk(&db, &trade_id);
        assert_eq!(status, "no_stop_loss");
    }

    #[test]
    fn manual_risk_status_persisted() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let service = svc(&db);
        let mut c = create_cmd(&a, &s, None);
        c.manual_risk = Some(500.0);
        let trade_id = service
            .execute(&env(command_type::CREATE_TRADE, c))
            .unwrap()[0]
            .payload["trade_id"]
            .as_str()
            .unwrap()
            .to_string();
        add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 110.0, 1.0);
        let (status, pnl, r) = trade_risk(&db, &trade_id);
        assert_eq!(status, "manual_risk");
        assert_eq!(pnl, Some(1000.0));
        assert_eq!(r, Some(2.0)); // ۱۰۰۰ ÷ ۵۰۰
    }

    // ==================== اجرای تخصیص‌نیافته ====================

    #[test]
    fn unassigned_execution_excluded_then_included_after_assignment() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        add_entry(&db, &trade_id, 100.0, 1.0);
        let exit_leg = add_exit(&db, &trade_id, 115.0, 1.0);

        // اجرای واردشده بدون تخصیص (جریان import: اول SourceRecord بعد Execution)
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO executions (id, trade_id, leg_id, leg_kind, source_record_id, kind,
                 direction, price, volume, assignment_status, created_at)
                 VALUES ('e1', NULL, NULL, NULL, NULL, 'imported', 'sell', 115.0, 1.0,
                 'needs_assignment', '2026-09-28T09:00:00Z')",
                [],
            )
            .unwrap();
        }
        // اجرای بی‌پیوند به این معامله وصل می‌شود ولی هنوز بدون پا؛
        // سپس بازمحاسبه (معادلِ پس‌پردازش ایمپورت) وضعیت را به‌روز می‌کند
        {
            let conn = db.lock();
            conn.execute("UPDATE executions SET trade_id = ?1 WHERE id = 'e1'", params![trade_id])
                .unwrap();
        }
        svc(&db).recompute_risk(&trade_id).unwrap();
        let (status, pnl, _) = trade_risk(&db, &trade_id);
        assert_eq!(status, "needs_assignment");
        assert_eq!(pnl, None);

        // تخصیص به پای خروج → R برمی‌گردد
        let service = svc(&db);
        let events = service
            .execute(&env(command_type::ASSIGN_EXECUTION_TO_LEG, AssignExecutionToLegCommand {
                execution_id: "e1".into(),
                leg_id: exit_leg,
            }))
            .unwrap();
        assert_eq!(events[0].event_type, "domain.execution_assigned");
        let (status2, pnl2, r2) = trade_risk(&db, &trade_id);
        assert_eq!(status2, "calculated");
        assert_eq!(pnl2, Some(1500.0));
        assert_eq!(r2, Some(1.5));
        // جهت اجرای وارداتی در لحظه تخصیص به جهت قطعی معامله نرمال شد
        let normalized: String = {
            let conn = db.lock();
            conn.query_row("SELECT direction FROM executions WHERE id = 'e1'", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(normalized, "buy");
        // حسابرسی تخصیص ثبت شده است
        let audit_count: i64 = {
            let conn = db.lock();
            conn.query_row(
                "SELECT COUNT(*) FROM audit_logs WHERE action = 'execution.assign' AND target = ?1",
                params![trade_id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(audit_count, 1);
    }

    #[test]
    fn assigned_execution_direction_mismatch_rejected_1407() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0)); // buy
        let leg = add_entry(&db, &trade_id, 100.0, 1.0);
        // اجرای assigned با جهت مخالف — داده معیوب باید در ورودی رد شود
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO executions (id, trade_id, kind, direction, price, volume, assignment_status, created_at)
                 VALUES ('e2', ?1, 'manual', 'sell', 100.0, 1.0, 'assigned', '2026-09-28T09:00:00Z')",
                params![trade_id],
            )
            .unwrap();
        }
        let service = svc(&db);
        let err = service
            .execute(&env(command_type::ASSIGN_EXECUTION_TO_LEG, AssignExecutionToLegCommand {
                execution_id: "e2".into(),
                leg_id: leg,
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1407);
    }

    #[test]
    fn manual_leg_with_executed_price_creates_manual_execution() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        let entry_leg = add_entry(&db, &trade_id, 100.0, 1.0);
        let exit_leg = add_exit(&db, &trade_id, 115.0, 1.0);
        let execs = svc(&db).executions(&trade_id).unwrap();
        assert_eq!(execs.len(), 2);
        assert!(execs.iter().all(|e| e.kind == "manual" && e.assignment_status == "assigned"));
        assert!(execs.iter().any(|e| e.leg_id.as_deref() == Some(entry_leg.as_str())
            && e.leg_kind == Some(LegKind::Entry)));
        assert!(execs.iter().any(|e| e.leg_id.as_deref() == Some(exit_leg.as_str())
            && e.leg_kind == Some(LegKind::Exit)));
        // هم‌گام‌سازی ویرایش پا با اجرای دستی
        let service = svc(&db);
        service
            .execute(&env(command_type::UPDATE_ENTRY_LEG, UpdateEntryLegCommand {
                leg_id: entry_leg,
                planned_price: None,
                executed_price: Some(101.0),
                volume: Some(2.0),
                stop_loss: None,
                take_profit: None,
                entry_time: None,
                note: None,
            }))
            .unwrap();
        let execs = service.executions(&trade_id).unwrap();
        let entry_exec = execs.iter().find(|e| e.leg_kind == Some(LegKind::Entry)).unwrap();
        assert_eq!(entry_exec.price, 101.0);
        assert_eq!(entry_exec.volume, 2.0);
    }

    #[test]
    fn soft_deleted_trade_hides_legs_executions_overrides_and_effective() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        let leg = add_entry(&db, &trade_id, 100.0, 1.0);
        let service = svc(&db);
        service
            .execute(&env(command_type::ADD_MANUAL_OVERRIDE, AddManualOverrideCommand {
                entity_type: "journal_trade".into(),
                entity_id: trade_id.clone(),
                field_name: "note".into(),
                previous_value: None,
                new_value: Some("یادداشت".into()),
                reason: Some("اصلاح".into()),
                source: "user".into(),
                priority: 1,
                reversible: true,
                created_by: "tester".into(),
            }))
            .unwrap();
        assert!(!service.entry_legs(&trade_id).unwrap().is_empty());
        assert!(service.effective_entity("journal_trade", &trade_id).unwrap().is_some());
        assert_eq!(service.overrides_of("journal_trade", &trade_id, false).unwrap().len(), 1);

        // حذف نرم
        service
            .execute(&env(command_type::DELETE_TRADE, DeleteTradeCommand {
                trade_id: trade_id.clone(),
                reason: Some("test".into()),
            }))
            .unwrap();

        // همه خواندنی‌های فرعی خالی/یافت‌نشده می‌شوند
        assert!(service.entry_legs(&trade_id).unwrap().is_empty());
        assert!(service.executions(&trade_id).unwrap().is_empty());
        assert!(service.overrides_of("journal_trade", &trade_id, false).unwrap().is_empty());
        assert!(service.effective_entity("journal_trade", &trade_id).unwrap().is_none());

        // ویرایش پا و بازنویسی جدید روی معامله حذف‌شده رد می‌شود (۱۴۰۱)
        let err = service
            .execute(&env(command_type::UPDATE_ENTRY_LEG, UpdateEntryLegCommand {
                leg_id: leg,
                planned_price: None,
                executed_price: Some(102.0),
                volume: None,
                stop_loss: None,
                take_profit: None,
                entry_time: None,
                note: None,
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1401);
        let err = service
            .execute(&env(command_type::ADD_MANUAL_OVERRIDE, AddManualOverrideCommand {
                entity_type: "journal_trade".into(),
                entity_id: trade_id.clone(),
                field_name: "note".into(),
                previous_value: None,
                new_value: Some("x".into()),
                reason: None,
                source: "user".into(),
                priority: 1,
                reversible: true,
                created_by: "tester".into(),
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1401);
    }

    #[test]
    fn assignment_conflict_across_trades_rejected() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let t1 = create_trade(&db, &a, &s, Some(90.0));
        let t2 = create_trade(&db, &a, &s, Some(90.0));
        let leg1 = add_entry(&db, &t1, 100.0, 1.0);
        add_entry(&db, &t2, 101.0, 1.0);
        // اجرای متعلق به t2 را نمی‌توان به پای t1 داد
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO executions (id, trade_id, kind, direction, price, volume, assignment_status, created_at)
                 VALUES ('e9', ?1, 'manual', 'buy', 100.0, 1.0, 'assigned', '2026-09-28T09:00:00Z')",
                params![t2],
            )
            .unwrap();
        }
        let service = svc(&db);
        let err = service
            .execute(&env(command_type::ASSIGN_EXECUTION_TO_LEG, AssignExecutionToLegCommand {
                execution_id: "e9".into(),
                leg_id: leg1,
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1407);
    }

    // ==================== به‌روزرسانی و حذف ====================

    #[test]
    fn update_trade_changes_basis_and_recomputes() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 115.0, 1.0);

        let c = UpdateTradeCommand {
            trade_id: trade_id.clone(),
            manual_risk: Some(3000.0),
            ..Default::default()
        };
        svc(&db)
            .execute(&env(command_type::UPDATE_TRADE, c))
            .unwrap();
        let (status, _, r) = trade_risk(&db, &trade_id);
        assert_eq!(status, "manual_risk");
        assert_eq!(r, Some(0.5)); // ۱۵۰۰ ÷ ۳۰۰۰

        // به‌روزرسانی خالی → خطا
        let empty = UpdateTradeCommand {
            trade_id: trade_id.clone(),
            ..Default::default()
        };
        let err = svc(&db)
            .execute(&env(command_type::UPDATE_TRADE, empty))
            .unwrap_err();
        assert_eq!(err.code(), 1404);
    }

    #[test]
    fn delete_trade_is_soft_and_audited() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        svc(&db)
            .execute(&env(command_type::DELETE_TRADE, DeleteTradeCommand {
                trade_id: trade_id.clone(),
                reason: Some("خطای ثبت دوبله".into()),
            }))
            .unwrap();

        let service = svc(&db);
        assert!(service.get_trade(&trade_id).unwrap().is_none());
        assert_eq!(service.list_trades(&a, false).unwrap().len(), 0);
        assert_eq!(service.list_trades(&a, true).unwrap().len(), 1);
        let conn = db.lock();
        let deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM journal_trades WHERE id = ?1",
                params![trade_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(deleted_at.is_some());
        let audits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_logs WHERE action = 'trade.delete' AND target = ?1",
                params![trade_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(audits, 1);
    }

    // ==================== بازنویسی دستی و داده مؤثر ====================

    #[test]
    fn override_effective_data_then_revert() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        let service = svc(&db);

        let events = service
            .execute(&env(command_type::ADD_MANUAL_OVERRIDE, AddManualOverrideCommand {
                entity_type: "journal_trade".into(),
                entity_id: trade_id.clone(),
                field_name: "strategy".into(),
                previous_value: Some("\"breakout\"".into()),
                new_value: Some("\"reversal\"".into()),
                reason: Some("بازنگری تحلیل".into()),
                source: "user".into(),
                priority: 10,
                reversible: true,
                created_by: "taha".into(),
            }))
            .unwrap();
        assert_eq!(events[0].event_type, "domain.override_added");

        let eff = service.effective_entity("journal_trade", &trade_id).unwrap().unwrap();
        assert_eq!(eff["strategy"], "reversal"); // مؤثر = بازنویسی
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        assert_eq!(trade.strategy.as_deref(), Some("breakout")); // canonical دست‌نخورده

        let ov_id = events[0].payload["override_id"].as_str().unwrap().to_string();
        service
            .execute(&env(command_type::REVERT_MANUAL_OVERRIDE, RevertManualOverrideCommand {
                override_id: ov_id,
            }))
            .unwrap();
        let eff2 = service.effective_entity("journal_trade", &trade_id).unwrap().unwrap();
        assert_eq!(eff2["strategy"], "breakout"); // بعد از بازگشت
        assert_eq!(service.overrides_of("journal_trade", &trade_id, false).unwrap().len(), 0);
        assert_eq!(service.overrides_of("journal_trade", &trade_id, true).unwrap().len(), 1);
    }

    #[test]
    fn non_reversible_override_cannot_be_reverted() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        let service = svc(&db);
        let events = service
            .execute(&env(command_type::ADD_MANUAL_OVERRIDE, AddManualOverrideCommand {
                entity_type: "journal_trade".into(),
                entity_id: trade_id.clone(),
                field_name: "note".into(),
                previous_value: None,
                new_value: Some("\"یادداشت\"".into()),
                reason: None,
                source: "user".into(),
                priority: 0,
                reversible: false,
                created_by: "taha".into(),
            }))
            .unwrap();
        let ov_id = events[0].payload["override_id"].as_str().unwrap().to_string();
        let err = service
            .execute(&env(command_type::REVERT_MANUAL_OVERRIDE, RevertManualOverrideCommand {
                override_id: ov_id,
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1408);
    }

    #[test]
    fn override_on_leg_changes_effective_data_not_canonical_r() {
        // بازنویسی دستی قیمت اجرای پای ورود → داده مؤثر عوض می‌شود؛
        // ستون‌های canonical و R پاید‌شده دست‌نخورده می‌مانند (طبق قرارداد لایه‌های داده،
        // داده مؤثر = canonical + overrides در لایه نمایش/فیلتر/آمار).
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        let leg_id = add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 115.0, 1.0);

        svc(&db)
            .execute(&env(command_type::ADD_MANUAL_OVERRIDE, AddManualOverrideCommand {
                entity_type: "entry_leg".into(),
                entity_id: leg_id.clone(),
                field_name: "executed_price".into(),
                previous_value: Some("100.0".into()),
                new_value: Some("102.0".into()),
                reason: Some("اصلاح قیمت ثبت‌شده".into()),
                source: "user".into(),
                priority: 5,
                reversible: true,
                created_by: "taha".into(),
            }))
            .unwrap();

        let service = svc(&db);
        let eff = service.effective_entity("entry_leg", &leg_id).unwrap().unwrap();
        assert_eq!(eff["executed_price"], 102.0);
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        assert_eq!(trade.realized_pnl, Some(1500.0)); // canonical
    }

    #[test]
    fn override_on_unknown_entity_rejected() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        let err = svc(&db)
            .execute(&env(command_type::ADD_MANUAL_OVERRIDE, AddManualOverrideCommand {
                entity_type: "attachment".into(),
                entity_id: trade_id,
                field_name: "note".into(),
                previous_value: None,
                new_value: None,
                reason: None,
                source: "user".into(),
                priority: 0,
                reversible: true,
                created_by: "taha".into(),
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1404);
    }

    // ==================== پیوست ====================

    #[test]
    fn link_attachment_to_trade() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO attachments (id, file_name, file_path, size_bytes, blake3_hash, created_at)
                 VALUES ('f1','chart.png','/x/chart.png',10,'hash','2026-09-28T10:00:00Z')",
                [],
            )
            .unwrap();
        }
        svc(&db)
            .execute(&env(command_type::LINK_ATTACHMENT_TO_TRADE, LinkAttachmentToTradeCommand {
                attachment_id: "f1".into(),
                trade_id: trade_id.clone(),
                link_kind: "chart".into(),
            }))
            .unwrap();
        let conn = db.lock();
        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM attachment_trade_links
                 WHERE trade_id = ?1 AND attachment_id = 'f1' AND link_kind = 'chart'",
                params![trade_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
    }

    #[test]
    fn link_attachment_rejects_unknown_attachment_or_kind() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        let service = svc(&db);
        let err = service
            .execute(&env(command_type::LINK_ATTACHMENT_TO_TRADE, LinkAttachmentToTradeCommand {
                attachment_id: "nope".into(),
                trade_id: trade_id.clone(),
                link_kind: "chart".into(),
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1409);

        let err2 = service
            .execute(&env(command_type::LINK_ATTACHMENT_TO_TRADE, LinkAttachmentToTradeCommand {
                attachment_id: "f1".into(),
                trade_id,
                link_kind: "before_entry".into(),
            }))
            .unwrap_err();
        assert_eq!(err2.code(), 1404);
    }

    // ==================== outbox ====================

    #[test]
    fn outbox_publish_flow_is_idempotent() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let _ = create_trade(&db, &a, &s, None);
        let service = svc(&db);
        let pending = service.unpublished_events(100).unwrap();
        assert!(!pending.is_empty());
        // event_id از outbox حفظ می‌شود (idempotency مصرف‌کننده)
        let ids: Vec<Uuid> = pending.iter().map(|e| e.event_id).collect();
        assert_eq!(ids.len(), service.mark_events_published(&ids).unwrap());
        // انتشار دوباره → صفر (idempotent)
        assert_eq!(service.mark_events_published(&ids).unwrap(), 0);
        assert_eq!(service.unpublished_events(100).unwrap().len(), 0);
    }

    #[test]
    fn leg_update_recomputes_risk() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        let leg = add_entry(&db, &trade_id, 100.0, 1.0);
        add_exit(&db, &trade_id, 115.0, 1.0);

        // اصلاح قیمت اجرای ورود به ۱۰۵ → ریسک = ۱۵ × ۱۰۰ = ۱۵۰۰
        svc(&db)
            .execute(&env(command_type::UPDATE_ENTRY_LEG, UpdateEntryLegCommand {
                leg_id: leg,
                planned_price: None,
                executed_price: Some(105.0),
                volume: None,
                stop_loss: None,
                take_profit: None,
                entry_time: None,
                note: None,
            }))
            .unwrap();
        let service = svc(&db);
        let trade = service.get_trade(&trade_id).unwrap().unwrap();
        assert_eq!(trade.initial_risk_amount, Some(1500.0));
        assert_eq!(trade.realized_pnl, Some(1000.0)); // (۱۱۵−۱۰۵)×۱×۱۰۰
        assert!((trade.trade_r.unwrap() - (1000.0 / 1500.0)).abs() < 1e-9);
    }

    #[test]
    fn exit_leg_update_recomputes_risk() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, Some(90.0));
        add_entry(&db, &trade_id, 100.0, 1.0);
        let leg = add_exit(&db, &trade_id, 115.0, 1.0);

        svc(&db)
            .execute(&env(command_type::UPDATE_EXIT_LEG, UpdateExitLegCommand {
                leg_id: leg,
                exit_reason: Some("stop".into()),
                executed_price: Some(95.0),
                volume: None,
                exit_time: None,
                note: None,
            }))
            .unwrap();
        let (_, pnl, r) = trade_risk(&db, &trade_id);
        assert_eq!(pnl, Some(-500.0));
        assert_eq!(r, Some(-0.5));
    }

    #[test]
    fn invalid_leg_volume_rejected_atomically() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        let service = svc(&db);
        let err = service
            .execute(&env(command_type::ADD_ENTRY_LEG, AddEntryLegCommand {
                trade_id: trade_id.clone(),
                planned_price: None,
                executed_price: Some(100.0),
                volume: -1.0,
                stop_loss: None,
                take_profit: None,
                entry_time: None,
                note: None,
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1404);
        assert!(service.entry_legs(&trade_id).unwrap().is_empty());
    }

    #[test]
    fn legs_of_deleted_trade_rejected() {
        let db = db();
        let (a, s) = setup_account_symbol(&db);
        let trade_id = create_trade(&db, &a, &s, None);
        svc(&db)
            .execute(&env(command_type::DELETE_TRADE, DeleteTradeCommand {
                trade_id: trade_id.clone(),
                reason: None,
            }))
            .unwrap();
        let err = svc(&db)
            .execute(&env(command_type::ADD_ENTRY_LEG, AddEntryLegCommand {
                trade_id,
                planned_price: None,
                executed_price: Some(100.0),
                volume: 1.0,
                stop_loss: None,
                take_profit: None,
                entry_time: None,
                note: None,
            }))
            .unwrap_err();
        assert_eq!(err.code(), 1401);
    }
}
