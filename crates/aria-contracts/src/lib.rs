//! aria-contracts — قراردادهای عمومی کرنل آریا
//!
//! این کرت «منبع حقیقت» قراردادهای عمومی است و فقط شامل قراردادها، نوع‌ها،
//! تریت‌ها، اسکیماها و تعریف‌های نسخه‌گذاری است. هیچ منطق کسب‌وکاری مجاز نیست.
//!
//! ماژول‌ها:
//! - [`envelope`]: پاکت دستور و پاکت رویداد (نسخه‌دار)
//! - [`manifest`]: قرارداد مانیفست پلاگین
//! - [`rpc`]: قرارداد JSON-RPC 2.0 برای ارتباط با پلاگین‌ها
//! - [`traits`]: تریت‌های سرویس‌های کرنل
//! - [`errors`]: قرارداد خطا (بازه‌های کد و ساختار بار خطای RPC)
//! - [`version`]: سیاست نسخه‌گذاری و سازگاری

pub mod envelope;
pub mod errors;
pub mod manifest;
pub mod rpc;
pub mod traits;
pub mod version;

pub use envelope::{CommandEnvelope, EventEnvelope, EventSource};
pub use errors::{ContractError, RpcErrorPayload};
pub use manifest::{PluginManifest, RuntimeMode, TrustLevel};
pub use rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, JsonRpcErrorObject, SessionToken, MethodDescriptor, RpcProtocolLimits};
pub use version::{
    BACKUP_FORMAT_VERSION, DATABASE_SCHEMA_VERSION, KERNEL_CONTRACT_VERSION, PLUGIN_API_VERSION,
    is_api_compatible, is_kernel_compatible,
};

#[cfg(test)]
mod tests {
    use crate::version;

    #[test]
    fn crate_smoke() {
        assert_eq!(version::DATABASE_SCHEMA_VERSION, 2);
    }
}
