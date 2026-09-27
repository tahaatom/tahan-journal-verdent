//! تریت‌های سرویس‌های کرنل — قراردادهای عمومی بین موتورها.
//!
//! قوانین:
//! - موتورها فقط از طریق این تریت‌ها با هم صحبت می‌کنند.
//! - پلاگین‌ها فقط از طریق PluginHostServices (بعد از بررسی مجوز) سرویس می‌گیرند.
//! - پیاده‌سازی‌ها نباید وضعیت خصوصی موتورهای دیگر را مستقیماً ببینند.

use crate::errors::{ContractError, RpcErrorPayload};
use serde_json::Value;

/// سرویس مدیریت کلید (موتور امنیت).
pub trait KeyProvider: Send + Sync {
    /// بازکردن پروفایل با گذرواژه — کلید فعال در حافظه امن نگه داشته می‌شود.
    fn unlock_profile(&self, profile_id: &str, password: &str) -> Result<(), ContractError>;
    /// قفل پروفایل — دسترسی به کلید فعال قطع می‌شود (نه فقط UI).
    fn lock_profile(&self, profile_id: &str) -> Result<(), ContractError>;
    /// قفل همه پروفایل‌ها.
    fn lock_all(&self);
    /// آیا پروفایل باز است.
    fn is_unlocked(&self, profile_id: &str) -> bool;
}

/// سرویس رمزنگاری (موتور امنیت).
pub trait EncryptionProvider: Send + Sync {
    /// رمزنگاری داده با AES-GCM و فراداده احرازشده.
    fn encrypt(&self, profile_id: &str, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, ContractError>;
    /// رمزگشایی داده (تشخیص دستکاری تضمین‌شده).
    fn decrypt(&self, profile_id: &str, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, ContractError>;
}

/// سرویس اسکیما (موتور فیلدها).
pub trait SchemaProvider: Send + Sync {
    /// فهرست فراداده فیلدهای سفارشی (برای فرم/فیلتر/لیست/داشبورد).
    fn list_fields(&self, include_inactive: bool) -> Result<Value, ContractError>;
    /// فراداده یک فیلد.
    fn get_field(&self, field_id: &str) -> Result<Value, ContractError>;
    /// فراداده رندر فرم (گروه، ترتیب، ویجت، اعتبارسنجی، گزینه‌ها).
    fn form_metadata(&self) -> Result<Value, ContractError>;
}

/// سرویس پرس‌وجو (موتور پرس‌وجو).
pub trait QueryProvider: Send + Sync {
    /// اجرای پرس‌وجوی فیلتر/تجمیع روی داده مؤثر. خروجی همیشه صفحه‌بندی‌شده است.
    fn query(&self, query: Value) -> Result<Value, ContractError>;
    /// خلاصه داشبورد از پروجکشن/کش (بدون محاسبه سنگین روی داده خام).
    fn dashboard_summary(&self) -> Result<Value, ContractError>;
}

/// سرویس حسابرسی (موتور امنیت).
pub trait AuditProvider: Send + Sync {
    /// ثبت رویداد حساس (login/logout/lock/unlock/backup/plugin/delete/...).
    fn record(&self, action: &str, actor: &str, target: &str, detail: Option<Value>) -> Result<(), ContractError>;
    /// خواندن لاگ حسابرسی (برای نمایش به کاربر).
    fn read_recent(&self, limit: usize) -> Result<Value, ContractError>;
}

/// سرویس بکاپ (موتور ذخیره‌سازی + امنیت).
pub trait BackupProvider: Send + Sync {
    /// ساخت بکاپ رمزنگاری‌شده کامل.
    fn create_backup(&self, destination: &str) -> Result<Value, ContractError>;
    /// بازیابی بکاپ با اعتبارسنجی کامل.
    fn restore_backup(&self, source: &str) -> Result<(), ContractError>;
}

/// سرویس پیوست‌ها (موتور ذخیره‌سازی + دامنه).
pub trait AttachmentProvider: Send + Sync {
    /// افزودن پیوست (هش blake3، تشخیص تکرار، تولید بندانگشتی).
    fn add_attachment(&self, file_path: &str, kind: &str) -> Result<Value, ContractError>;
    /// اتصال پیوست به معامله با نوع پیوند.
    fn link_to_trade(&self, attachment_id: &str, trade_id: &str, link_kind: &str) -> Result<(), ContractError>;
    /// فهرست پیوست‌های یک معامله.
    fn list_for_trade(&self, trade_id: &str) -> Result<Value, ContractError>;
}

/// سرویس دامنه معاملاتی (موتور دامنه).
pub trait TradeServiceProvider: Send + Sync {
    /// اجرای دستور دامنه (CreateTrade/UpdateTrade/...). اتمیک؛ رویدادها پس از commit.
    fn execute_command(&self, command_type: &str, payload: Value) -> Result<Value, RpcErrorPayload>;
    /// دریافت معامله با داده مؤثر (کاننیکل + بازنویسی‌ها).
    fn get_trade_effective(&self, trade_id: &str) -> Result<Value, RpcErrorPayload>;
}

/// سرویس فیلدهای سفارشی (موتور فیلدها).
pub trait FieldServiceProvider: Send + Sync {
    /// تعریف فیلد جدید (بدون کد).
    fn define_field(&self, definition: Value) -> Result<Value, RpcErrorPayload>;
    /// به‌روزرسانی فیلد (تغییر نوع کنترل‌شده و مهاجرت‌آگاه).
    fn update_field(&self, field_id: &str, changes: Value) -> Result<Value, RpcErrorPayload>;
    /// غیرفعال‌سازی فیلد (حذف سخت برای فیلد دارای داده ممنوع).
    fn deactivate_field(&self, field_id: &str) -> Result<(), RpcErrorPayload>;
    /// ذخیره مقدار فیلد برای معامله.
    fn set_value(&self, trade_id: &str, field_id: &str, value: Value) -> Result<(), RpcErrorPayload>;
    /// خواندن مقادیر فیلد یک معامله.
    fn get_values(&self, trade_id: &str) -> Result<Value, RpcErrorPayload>;
}

/// سرویس‌های میزبان پلاگین — تنها نقطه دسترسی پلاگین‌ها به کرنل.
///
/// هر فراخوانی قبل از اجرا از نظر «مجوز قابلیت» بررسی می‌شود؛
/// دسترسی مستقیم به پایگاه‌داده/فایل‌سیستم/کلیدها از این مسیر وجود ندارد.
pub trait PluginHostServices: Send + Sync {
    /// پرس‌وجوی مجاز (نیازمند قابلیت مربوطه).
    fn query(&self, session: &str, query: Value) -> Result<Value, RpcErrorPayload>;
    /// دستور دامنه مجاز (نیازمند قابلیت مربوطه).
    fn trade_command(&self, session: &str, command_type: &str, payload: Value) -> Result<Value, RpcErrorPayload>;
    /// خواندن آمار (نیازمند stats.read).
    fn stats(&self, session: &str) -> Result<Value, RpcErrorPayload>;
    /// نوشتن بینش (نیازمند insights.write).
    fn write_insight(&self, session: &str, insight: Value) -> Result<(), RpcErrorPayload>;
    /// ارسال اعلان به UI (نیازمند notifications.show).
    fn notify(&self, session: &str, message: Value) -> Result<(), RpcErrorPayload>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// پیاده‌سازی نمونه AuditProvider برای اثبات object-safety و استفاده از تریت.
    struct FakeAudit(Mutex<Vec<String>>);
    impl AuditProvider for FakeAudit {
        fn record(&self, action: &str, actor: &str, target: &str, _detail: Option<Value>) -> Result<(), ContractError> {
            self.0.lock().unwrap().push(format!("{action}:{actor}:{target}"));
            Ok(())
        }
        fn read_recent(&self, limit: usize) -> Result<Value, ContractError> {
            let v = self.0.lock().unwrap();
            Ok(Value::Array(v.iter().take(limit).map(|s| Value::String(s.clone())).collect()))
        }
    }

    #[test]
    fn traits_are_object_safe_and_usable() {
        let audit: Box<dyn AuditProvider> = Box::new(FakeAudit(Mutex::new(vec![])));
        audit.record("login", "ui", "profile-1", None).unwrap();
        audit.record("trade_delete", "ui", "trade-9", None).unwrap();
        let recent = audit.read_recent(10).unwrap();
        assert_eq!(recent.as_array().unwrap().len(), 2);
    }
}
