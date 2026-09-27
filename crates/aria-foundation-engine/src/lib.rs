//! aria-foundation-engine — موتور پایه کرنل آریا
//!
//! ماژول‌ها:
//! - [`error`]: مدل خطای ساخت‌یافته کرنل با کد عددی یکتا، واریانت ماشین‌خوان و کلید پیام فارسی
//! - [`ids`]: شناسه‌های نوع‌دار موجودیت‌ها (uuid v4)
//! - [`time`]: سرویس زمان — ذخیره UTC/ISO 8601، تقویم جلالی برای نمایش فارسی، سشن معاملاتی
//! - [`config`]: پیکربندی کرنل (مسیرها، زبان، تم، قفل خودکار)
//! - [`logging`]: لاگ‌سازی محلی ساخت‌یافته بدون اسرار

pub mod config;
pub mod error;
pub mod ids;
pub mod logging;
pub mod time;

pub use config::KernelConfig;
pub use error::{FoundationError, KernelError};
pub use ids::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_smoke() {
        let id = TradeId::new();
        assert!(!id.to_string().is_empty());
    }
}
