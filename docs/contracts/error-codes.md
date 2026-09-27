# کدهای خطا — قرارداد پایدار

این سند مرجع رسمی کدهای عددی خطاهای کرنل آریاست. کدها **پایدار** هستند و نباید تغییر کنند؛
منسوخ‌سازی فقط با افزودن واریانت جدید انجام می‌شود.

## بازه‌های اختصاص‌یافته

| بازه | موتور | وضعیت |
|---|---|---|
| 1000–1099 | aria-foundation-engine | فعال |
| 1100–1199 | aria-storage-engine | رزرو (فاز ۱.۳) |
| 1200–1299 | aria-security-engine | رزرو (فاز ۱.۴) |
| 1300–1399 | aria-schema-engine | رزرو (فاز ۱.۵) |
| 1400–1499 | aria-domain-engine | رزرو (فاز ۱.۶) |
| 1500–1599 | aria-plugin-engine | رزرو (فاز ۱.۷) |
| 1600–1699 | aria-runtime-engine | رزرو (فاز ۱.۸) |
| 1700–1799 | aria-query-engine | رزرو (فاز ۱.۹) |
| 1800–1899 | aria-ui-engine | رزرو (فاز ۱.۱۰) |

## کدهای فعلی موتور پایه

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1001 | `config_load_failed` | `error.foundation.config_load_failed` | بارگذاری فایل پیکربندی ناموفق |
| 1002 | `config_parse_failed` | `error.foundation.config_parse_failed` | پارس پیکربندی ناموفق |
| 1003 | `config_save_failed` | `error.foundation.config_save_failed` | ذخیره پیکربندی ناموفق |
| 1004 | `io` | `error.foundation.io` | خطای ورودی/خروجی عمومی |
| 1005 | `invalid_date_time` | `error.foundation.invalid_date_time` | تاریخ/زمان نامعتبر |
| 1006 | `invalid_config_value` | `error.foundation.invalid_config_value` | مقدار پیکربندی نامعتبر |

## قواعد

1. هر واریانت خطا باید کد یکتا، واریانت ماشین‌خوان و کلید پیام فارسی داشته باشد.
2. تست «یکتایی کدها» در هر موتور اجباری است.
3. کدها فقط اضافه می‌شوند؛ هرگز بازمقداردهی یا حذف نمی‌شوند.
