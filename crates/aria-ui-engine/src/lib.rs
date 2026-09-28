//! aria-ui-engine — موتور رابط کاربری کرنل آریا
//!
//! شامل: رجیستری افزونه‌پذیری UI به‌صورت اعلانی، فراداده فرم‌ها
//! و رندر اسکیماهای اعلانی پلاگین‌ها.
//!
//! پیاده‌سازی کامل در فاز ۱.۱۰ انجام می‌شود.

/// نسخه ساختارهای اعلانی UI (رزرو فاز ۱.۱۰).
pub const UI_ENGINE_DECL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(super::UI_ENGINE_DECL_VERSION, 1);
    }
}
