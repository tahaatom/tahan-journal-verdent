//! قرارداد JSON-RPC 2.0 برای ارتباط کرنل ↔ پلاگین.
//!
//! - پیام‌ها newline-delimited JSON روی stdio یا IPC محلی.
//! - حداکثر اندازه پیام: 10 MB — مهلت پیش‌فرض: 30 ثانیه.
//! - احراز هویت با توکن نشست.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// حداکثر اندازه مجاز هر پیام RPC: 10 مگابایت.
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// مهلت پیش‌فرض پاسخ RPC: 30 ثانیه.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// نسخه پروتکل JSON-RPC.
pub const JSONRPC_VERSION: &str = "2.0";

/// توکن نشست RPC — فقط در حافظه، پس از ثبت پلاگین صادر می‌شود.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionToken(String);

impl SessionToken {
    /// ساخت توکن نشست جدید تصادفی.
    pub fn new() -> Self {
        Self(format!("sess_{}", Uuid::new_v4().simple()))
    }

    /// ساخت از مقدار موجود (برای بازیابی در حافظه).
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// مقدار توکن.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionToken {
    fn default() -> Self {
        Self::new()
    }
}

/// درخواست JSON-RPC 2.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// توکن نشست (خارج از مشخصات JSON-RPC؛ فیلد اختصاصی آریا)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionToken>,
}

impl JsonRpcRequest {
    /// ساخت درخواست جدید.
    pub fn new(id: serde_json::Value, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
            session: None,
        }
    }

    /// پیوست توکن نشست.
    pub fn with_session(mut self, token: SessionToken) -> Self {
        self.session = Some(token);
        self
    }
}

/// پاسخ موفق JSON-RPC 2.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub result: serde_json::Value,
}

impl JsonRpcResponse {
    pub fn new(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self { jsonrpc: JSONRPC_VERSION.to_string(), id, result }
    }
}

/// شیء خطای JSON-RPC 2.0 (کد، پیام، داده).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// پاسخ خطای JSON-RPC 2.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub error: JsonRpcErrorObject,
}

impl JsonRpcError {
    pub fn new(id: serde_json::Value, code: i32, message: impl Into<String>, data: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            error: JsonRpcErrorObject { code, message: message.into(), data },
        }
    }
}

/// کدهای استاندارد خطای JSON-RPC 2.0 + کدهای اختصاصی آریا.
pub mod rpc_error_codes {
    /// پارس نامعتبر
    pub const PARSE_ERROR: i32 = -32700;
    /// درخواست نامعتبر
    pub const INVALID_REQUEST: i32 = -32600;
    /// متد یافت نشد
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// پارامترهای نامعتبر
    pub const INVALID_PARAMS: i32 = -32602;
    /// خطای داخلی
    pub const INTERNAL_ERROR: i32 = -32603;
    /// پیام بزرگ‌تر از حد مجاز
    pub const MESSAGE_TOO_LARGE: i32 = -32000;
    /// نشست نامعتبر/منقضی
    pub const SESSION_INVALID: i32 = -32001;
    /// مجوز کافی نیست
    pub const PERMISSION_DENIED: i32 = -32002;
    /// تجاوز از مهلت
    pub const TIMEOUT: i32 = -32003;
}

/// توصیف‌گر متد در رجیستری متدهای سرویس‌پذیر پلاگین‌ها.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MethodDescriptor {
    /// نام متد (مثل "trades.list")
    pub method: String,
    /// شرح متد
    pub description: String,
    /// قابلیت لازم برای فراخوانی (مثل "trades.read")
    pub required_capability: String,
    /// آیا متد فقط خواندنی است
    pub read_only: bool,
}

impl MethodDescriptor {
    /// ساخت توصیف‌گر متد.
    pub fn new(method: impl Into<String>, description: impl Into<String>, required_capability: impl Into<String>, read_only: bool) -> Self {
        Self {
            method: method.into(),
            description: description.into(),
            required_capability: required_capability.into(),
            read_only,
        }
    }
}

/// محدودیت‌های پروتکل RPC (برای ردِ پیام‌های نامعتبر).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RpcProtocolLimits {
    pub max_message_size: usize,
    pub default_timeout_secs: u64,
}

impl Default for RpcProtocolLimits {
    fn default() -> Self {
        Self { max_message_size: MAX_MESSAGE_SIZE, default_timeout_secs: DEFAULT_TIMEOUT_SECS }
    }
}

impl RpcProtocolLimits {
    /// بررسی اندازه پیام (بایت).
    pub fn message_allowed(&self, len: usize) -> bool {
        len <= self.max_message_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_response_error_roundtrip() {
        let req = JsonRpcRequest::new(json!(1), "trades.list", Some(json!({"limit": 50})))
            .with_session(SessionToken::from_string("sess_abc"));
        let s = serde_json::to_string(&req).unwrap();
        let back: JsonRpcRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert_eq!(back.jsonrpc, "2.0");

        let res = JsonRpcResponse::new(json!(1), json!({"items": []}));
        let s = serde_json::to_string(&res).unwrap();
        let back: JsonRpcResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(res, back);

        let err = JsonRpcError::new(json!(1), rpc_error_codes::PERMISSION_DENIED, "denied", None);
        let s = serde_json::to_string(&err).unwrap();
        let back: JsonRpcError = serde_json::from_str(&s).unwrap();
        assert_eq!(err, back);
        assert_eq!(back.error.code, -32002);
    }

    #[test]
    fn session_token_unique_and_display() {
        let a = SessionToken::new();
        let b = SessionToken::new();
        assert_ne!(a, b);
        assert!(a.as_str().starts_with("sess_"));
    }

    #[test]
    fn message_size_limit_enforced() {
        let limits = RpcProtocolLimits::default();
        assert!(limits.message_allowed(1000));
        assert!(!limits.message_allowed(MAX_MESSAGE_SIZE + 1));
        assert_eq!(limits.default_timeout_secs, 30);
    }

    #[test]
    fn method_descriptor_serde() {
        let d = MethodDescriptor::new("stats.dashboard", "داشبورد", "stats.read", true);
        let s = serde_json::to_string(&d).unwrap();
        let back: MethodDescriptor = serde_json::from_str(&s).unwrap();
        assert_eq!(d, back);
    }
}
