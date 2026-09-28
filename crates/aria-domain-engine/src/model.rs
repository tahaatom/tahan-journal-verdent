//! مدل دامنه معاملاتی — موجودیت‌های نسخه ۱.

use serde::{Deserialize, Serialize};

/// جهت معامله.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Buy,
    Sell,
}

impl Direction {
    /// نمای رشته‌ای پایگاه‌داده.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    /// خواندن از نمای رشته‌ای.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "buy" => Some(Self::Buy),
            "sell" => Some(Self::Sell),
            _ => None,
        }
    }

    /// علامت PnL: خرید → +۱، فروش → −۱.
    pub fn pnl_sign(&self) -> f64 {
        match self {
            Self::Buy => 1.0,
            Self::Sell => -1.0,
        }
    }
}

/// وضعیت معامله.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeStatus {
    Open,
    Closed,
    Cancelled,
}

impl TradeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// نوع پا: ورود یا خروج.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    Entry,
    Exit,
}

impl LegKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Exit => "exit",
        }
    }
}

/// ژورنال معامله — واحد تصمیم مستقل (رکورد canonical).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Trade {
    pub id: String,
    pub account_id: String,
    pub symbol_id: String,
    pub direction: Direction,
    pub status: TradeStatus,
    pub strategy: Option<String>,
    pub timeframe: Option<String>,
    pub session: Option<String>,
    pub market_condition: Option<String>,
    pub entry_type: Option<String>,
    pub note: Option<String>,
    pub tags: Option<String>,
    pub emotions: Option<String>,
    pub mistakes: Option<String>,
    pub entry_time: Option<String>,
    pub exit_time: Option<String>,
    pub initial_stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub manual_risk: Option<f64>,
    pub risk_calculation_status: String,
    pub risk_basis: Option<String>,
    pub planned_r: Option<f64>,
    pub initial_risk_amount: Option<f64>,
    pub realized_pnl: Option<f64>,
    pub realized_r: Option<f64>,
    pub trade_r: Option<f64>,
    pub commission: f64,
    pub swap: f64,
    /// رزرو نسخه ۱ — گروه پوزیشن ضمنی؛ در نسخه ۳ فعال می‌شود
    pub position_group_id: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// پای ورود.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EntryLeg {
    pub id: String,
    pub trade_id: String,
    pub planned_price: Option<f64>,
    pub executed_price: Option<f64>,
    pub volume: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub entry_time: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// پای خروج.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExitLeg {
    pub id: String,
    pub trade_id: String,
    pub exit_reason: Option<String>,
    pub executed_price: Option<f64>,
    pub volume: f64,
    pub exit_time: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// اجرا — فیل واقعی بروکر یا دستی.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Execution {
    pub id: String,
    pub trade_id: Option<String>,
    pub leg_id: Option<String>,
    pub leg_kind: Option<LegKind>,
    pub source_record_id: Option<String>,
    pub kind: String,
    pub direction: Direction,
    pub price: f64,
    pub volume: f64,
    pub executed_at: Option<String>,
    pub commission: f64,
    pub swap: f64,
    pub ticket: Option<String>,
    pub magic: Option<String>,
    pub comment: Option<String>,
    /// assigned | needs_assignment
    pub assignment_status: String,
    pub created_at: String,
}

/// رکورد خام منبع — تغییرناپذیر.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRecord {
    pub id: String,
    pub import_batch_id: String,
    pub raw_payload: String,
    pub payload_hash: String,
    pub source: String,
    pub imported_at: String,
    /// pending | processed | failed | duplicate
    pub processed_status: String,
}

/// بازنویسی دستی — دفترکل بازنویسی با تاریخچه کامل.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ManualOverride {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub field_name: String,
    pub previous_value: Option<String>,
    pub new_value: Option<String>,
    pub reason: Option<String>,
    pub source: String,
    pub priority: i64,
    pub reversible: bool,
    pub created_by: String,
    pub created_at: String,
    pub reverted_at: Option<String>,
}

/// نگاشت رشته direction به enum با خطای دامنه‌ای مناسب (کمک سرویس).
pub(crate) fn parse_direction(s: &str) -> Result<Direction, crate::error::DomainError> {
    Direction::parse(s).ok_or_else(|| crate::error::DomainError::storage(format!("corrupt direction: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_roundtrip_and_sign() {
        assert_eq!(Direction::parse("buy"), Some(Direction::Buy));
        assert_eq!(Direction::parse("sell").unwrap().as_str(), "sell");
        assert_eq!(Direction::parse("x"), None);
        assert_eq!(Direction::Buy.pnl_sign(), 1.0);
        assert_eq!(Direction::Sell.pnl_sign(), -1.0);
    }

    #[test]
    fn status_roundtrip() {
        for s in ["open", "closed", "cancelled"] {
            let parsed = TradeStatus::parse(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert_eq!(TradeStatus::parse("other"), None);
    }

    #[test]
    fn serde_roundtrip_trade_direction() {
        let json = serde_json::to_string(&Direction::Buy).unwrap();
        let back: Direction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Direction::Buy);
    }
}
