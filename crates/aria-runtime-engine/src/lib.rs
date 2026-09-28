//! aria-runtime-engine — موتور اجرا کرنل آریا
//!
//! شامل: سوپروایزر فرایند سایدکار، پل JSON-RPC روی stdio،
//! پایش منابع و میزبان پلاگین‌های پایتون.
//!
//! پیاده‌سازی کامل در فاز ۱.۸ انجام می‌شود.

/// نسخه قرارداد ناظر فرایند (رزرو فاز ۱.۸).
pub const RUNTIME_SUPERVISOR_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(super::RUNTIME_SUPERVISOR_VERSION, 1);
    }
}
