//! قرارداد خطا: بازه‌های کد پایدار per-engine و ساختار بار خطای RPC.
//!
//! مرجع کامل: `docs/contracts/error-codes.md`.

use serde::{Deserialize, Serialize};

/// بازه‌های کد خطا — پایدار و فقط افزودنی.
pub mod ranges {
    pub const FOUNDATION: (u32, u32) = (1000, 1099);
    pub const STORAGE: (u32, u32) = (1100, 1199);
    pub const SECURITY: (u32, u32) = (1200, 1299);
    pub const SCHEMA: (u32, u32) = (1300, 1399);
    pub const DOMAIN: (u32, u32) = (1400, 1499);
    pub const PLUGIN: (u32, u32) = (1500, 1599);
    pub const RUNTIME: (u32, u32) = (1600, 1699);
    pub const QUERY: (u32, u32) = (1700, 1799);
    pub const UI: (u32, u32) = (1800, 1899);
}

/// بررسی تعلق کد به بازه مشخص.
pub fn code_in_range(code: u32, range: (u32, u32)) -> bool {
    code >= range.0 && code <= range.1
}

/// بار خطای RPC — قالب یکسان برای همه موتورها.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RpcErrorPayload {
    /// کد عددی پایدار کرنل
    pub code: u32,
    /// واریانت ماشین‌خوان
    pub variant: String,
    /// کلید پیام فارسی برای i18n متمرکز
    pub message_key: String,
    /// زمینه اختیاری (بدون داده حساس)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

impl RpcErrorPayload {
    /// ساخت بار خطا.
    pub fn new(code: u32, variant: impl Into<String>, message_key: impl Into<String>) -> Self {
        Self { code, variant: variant.into(), message_key: message_key.into(), context: None }
    }

    /// افزودن زمینه.
    pub fn with_context(mut self, ctx: serde_json::Value) -> Self {
        self.context = Some(ctx);
        self
    }
}

/// خطای قراردادها (درون این کرت).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ContractError {
    #[error("contract violated: {reason}")]
    Violation { reason: String },
    #[error("version incompatible: {reason}")]
    VersionIncompatible { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_disjoint() {
        let all = [
            ranges::FOUNDATION,
            ranges::STORAGE,
            ranges::SECURITY,
            ranges::SCHEMA,
            ranges::DOMAIN,
            ranges::PLUGIN,
            ranges::RUNTIME,
            ranges::QUERY,
            ranges::UI,
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                let overlap = all[i].0 <= all[j].1 && all[j].0 <= all[i].1;
                assert!(!overlap, "ranges {:?} and {:?} overlap", all[i], all[j]);
            }
        }
    }

    #[test]
    fn code_range_membership() {
        assert!(code_in_range(1001, ranges::FOUNDATION));
        assert!(!code_in_range(1101, ranges::FOUNDATION));
        assert!(code_in_range(1405, ranges::DOMAIN));
    }

    #[test]
    fn rpc_error_payload_roundtrip() {
        let e = RpcErrorPayload::new(1401, "trade_not_found", "error.domain.trade_not_found")
            .with_context(serde_json::json!({"trade_id": "x"}));
        let s = serde_json::to_string(&e).unwrap();
        let back: RpcErrorPayload = serde_json::from_str(&s).unwrap();
        assert_eq!(e, back);
    }
}
