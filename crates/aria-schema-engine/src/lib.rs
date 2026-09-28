//! aria-schema-engine — موتور فیلدهای سفارشی کرنل آریا
//!
//! فیلدهای تعریف‌شده توسط کاربر به‌عنوان «شهروند درجه‌یک تحلیلی» — بدون هیچ کدی:
//! - [`model`]: تعریف فیلد (نوع ذخیره‌سازی/معنایی، قواعد اعتبارسنجی، گزینه‌ها)
//! - [`validation`]: اعتبارسنجی مقادیر بر اساس نوع و قواعد — مبنای فرم/فیلتر/آمار
//! - [`service`]: سرویس CRUD فیلدها و مقادیر روی پایگاه‌داده + فراداده فرم + پل قرارداد
//!
//! قواعد نسخه ۱ (سند نسخه ۵):
//! - تغییر `storage_type` فقط تا وقتی فیلد داده ندارد مجاز است (تغییر کنترل‌شده و مهاجرت‌آگاه).
//! - حذف سخت فیلد دارای داده ممنوع — فقط غیرفعال‌سازی.
//! - گزینه‌های enum فقط افزودنی‌اند؛ گزینه استفاده‌شده فقط غیرفعال می‌شود.
//! - فیلدهای `filterable` در فیلترها، `stat_enabled` در آمار و `analysis_enabled` در تحلیل ظاهر می‌شوند.

pub mod error;
pub mod model;
pub mod service;
pub mod validation;

pub use error::SchemaError;
pub use model::{FieldDefinition, FieldOption, SemanticType, StorageType, ValidationRules};
pub use service::SchemaService;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_smoke() {
        assert!(crate::error::SCHEMA_ERROR_RANGE.0 >= 1300);
    }
}
