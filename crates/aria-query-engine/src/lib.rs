//! aria-query-engine — موتور پرس‌وجوی کرنل آریا
//!
//! شامل: فیلترهای ساده و مرکب، تجمیع‌ها، پروجکشن‌های داشبورد،
//! کش و تحلیل فیلدهای سفارشی.
//!
//! پیاده‌سازی کامل در فاز ۱.۹ انجام می‌شود.

/// نسخه قرارداد پرس‌وجو (رزرو فاز ۱.۹).
pub const QUERY_CONTRACT_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(super::QUERY_CONTRACT_VERSION, 1);
    }
}
