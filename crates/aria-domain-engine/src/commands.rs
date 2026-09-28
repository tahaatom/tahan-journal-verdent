//! بار دستورهای دامنه — ۱۱ دستور نسخه ۱.
//!
//! هر دستور از طریق `CommandEnvelope` با command_type معادل اجرا می‌شود.
//! مرجع: `docs/contracts/command-contract.md`

use serde::{Deserialize, Serialize};

/// نام‌های command_type نسخه ۱ (ثابت‌های پایدار).
pub mod command_type {
    pub const CREATE_TRADE: &str = "domain.create_trade";
    pub const UPDATE_TRADE: &str = "domain.update_trade";
    pub const DELETE_TRADE: &str = "domain.delete_trade";
    pub const ADD_ENTRY_LEG: &str = "domain.add_entry_leg";
    pub const UPDATE_ENTRY_LEG: &str = "domain.update_entry_leg";
    pub const ADD_EXIT_LEG: &str = "domain.add_exit_leg";
    pub const UPDATE_EXIT_LEG: &str = "domain.update_exit_leg";
    pub const ASSIGN_EXECUTION_TO_LEG: &str = "domain.assign_execution_to_leg";
    pub const ADD_MANUAL_OVERRIDE: &str = "domain.add_manual_override";
    pub const REVERT_MANUAL_OVERRIDE: &str = "domain.revert_manual_override";
    pub const LINK_ATTACHMENT_TO_TRADE: &str = "domain.link_attachment_to_trade";
}

/// CreateTradeCommand — ساخت معامله دستی.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateTradeCommand {
    pub account_id: String,
    pub symbol_id: String,
    /// "buy" | "sell"
    pub direction: String,
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
    pub initial_stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub manual_risk: Option<f64>,
    pub commission: f64,
    pub swap: f64,
}

/// UpdateTradeCommand — به‌روزرسانی فیلدهای قابل ویرایش.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateTradeCommand {
    pub trade_id: String,
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
    pub status: Option<String>,
    pub initial_stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub manual_risk: Option<f64>,
    pub commission: Option<f64>,
    pub swap: Option<f64>,
}

/// DeleteTradeCommand — حذف نرم (داده کاربر هرگز سخت‌حذف نمی‌شود).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeleteTradeCommand {
    pub trade_id: String,
    pub reason: Option<String>,
}

/// AddEntryLegCommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddEntryLegCommand {
    pub trade_id: String,
    pub planned_price: Option<f64>,
    pub executed_price: Option<f64>,
    pub volume: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub entry_time: Option<String>,
    pub note: Option<String>,
}

/// UpdateEntryLegCommand — حذف سخت ممنوع؛ فقط به‌روزرسانی.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateEntryLegCommand {
    pub leg_id: String,
    pub planned_price: Option<f64>,
    pub executed_price: Option<f64>,
    pub volume: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub entry_time: Option<String>,
    pub note: Option<String>,
}

/// AddExitLegCommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddExitLegCommand {
    pub trade_id: String,
    pub exit_reason: Option<String>,
    pub executed_price: Option<f64>,
    pub volume: f64,
    pub exit_time: Option<String>,
    pub note: Option<String>,
}

/// UpdateExitLegCommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateExitLegCommand {
    pub leg_id: String,
    pub exit_reason: Option<String>,
    pub executed_price: Option<f64>,
    pub volume: Option<f64>,
    pub exit_time: Option<String>,
    pub note: Option<String>,
}

/// AssignExecutionToLegCommand — تخصیص اجرای تخصیص‌نیافته به یک پا.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AssignExecutionToLegCommand {
    pub execution_id: String,
    pub leg_id: String,
}

/// AddManualOverrideCommand — بازنویسی دستی با دلیل و منبع.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddManualOverrideCommand {
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
}

/// RevertManualOverrideCommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RevertManualOverrideCommand {
    pub override_id: String,
}

/// LinkAttachmentToTradeCommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LinkAttachmentToTradeCommand {
    pub attachment_id: String,
    pub trade_id: String,
    /// before_trade | after_trade | chart | news | other
    pub link_kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_types_are_versioned_and_stable() {
        assert!(command_type::CREATE_TRADE.starts_with("domain."));
        assert_eq!(command_type::CREATE_TRADE, "domain.create_trade");
        assert_eq!(command_type::ASSIGN_EXECUTION_TO_LEG, "domain.assign_execution_to_leg");
    }

    #[test]
    fn create_trade_command_serde_roundtrip() {
        let cmd = CreateTradeCommand {
            account_id: "a".into(),
            symbol_id: "s".into(),
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
            entry_time: None,
            initial_stop_loss: Some(90.0),
            take_profit: None,
            manual_risk: None,
            commission: 0.0,
            swap: 0.0,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: CreateTradeCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(back.initial_stop_loss, Some(90.0));
        assert_eq!(back.strategy.as_deref(), Some("breakout"));
    }
}
