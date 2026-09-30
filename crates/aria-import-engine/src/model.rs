//! مدل‌های موتور ایمپورت — ردیف خام و معامله نگاشت‌شده (فاز ۱.۱۵).

use serde::{Deserialize, Serialize};

/// قالب فایل خروجی متاتریدر.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    /// CSV/TSV معاملات MT5 (ستون‌های Time/Position/Type/Direction/…)
    CsvDeals,
    /// گزارش HTML صورت‌حساب MT4 (جدول Closed Transactions)
    HtmlStatement,
}

/// جهت معامله نگاشت‌شده.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DealSide {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
}

impl DealSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

/// نقش معامله در پوزیشن — ورود، خروج یا نامشخص.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealRole {
    In,
    Out,
    Unknown,
}

/// یک ردیف نگاشت‌شده به فیلدهای کانونی (فاز ۱.۱۵).
#[derive(Debug, Clone, PartialEq)]
pub struct MappedDeal {
    /// اندیس ردیف خام مبدأ (برای پیام‌های خطا)
    pub row_index: usize,
    /// متن خام ردیف — حفظ بدون تغییر
    pub raw: String,
    /// هش blake3 ردیف خام
    pub payload_hash: String,
    /// شماره تیکت معامله/دیل
    pub ticket: Option<String>,
    /// شناسه پوزیشن — کلید گروه‌بندی دیل‌های یک معامله
    pub position_id: Option<String>,
    /// نام نماد بروکر (مثل XAUUSD)
    pub symbol: String,
    pub direction: DealSide,
    /// نقش ورود/خروج — از Direction (MT5) یا قالب فایل
    pub role: DealRole,
    /// زمان دیل/باز شدن — RFC3339 UTC
    pub time: String,
    /// زمان بستن — فقط قالب HTML خلاصه MT4
    pub close_time: Option<String>,
    pub price: f64,
    pub volume: f64,
    pub commission: f64,
    pub swap: f64,
    /// سود/زیان دیل — صفر برای دیل ورود MT5
    pub profit: f64,
    pub magic: Option<String>,
    pub comment: Option<String>,
}

/// گزارش کامل یک دسته ایمپورت — اعداد و هشدارهای فارسی.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    /// شناسه دسته ایمپورت
    pub batch_id: String,
    /// هش blake3 کل فایل
    pub file_hash: String,
    /// تعداد کل ردیف‌های شناسایی‌شده در فایل
    pub total_rows: usize,
    /// ردیف‌های غیرمعاملاتی رد‌شده (موجودی/اعتبار و ردیف خالی)
    pub skipped: usize,
    /// دیل‌های درج‌شده (اجراها)
    pub imported: usize,
    /// ردیف‌های تکراری نادیده‌گرفته‌شده
    pub duplicates: usize,
    /// ردیف‌های ناموفق (خطای نگاشت/درج)
    pub errors: usize,
    /// اجراهای در انتظار تخصیص کاربر
    pub needs_assignment: usize,
    /// معاملات جدید ساخته‌شده
    pub trades_created: usize,
    /// کل فایل تکراری بود؟ (همه ردیف‌ها قبلاً ایمپورت شده‌اند)
    pub file_duplicate: bool,
    /// هشدارهای تفصیلی فارسی
    pub warnings: Vec<String>,
}

impl ImportReport {
    /// افزودن هشدار فارسی.
    pub fn warn(&mut self, text: impl Into<String>) {
        self.warnings.push(text.into());
    }
}
