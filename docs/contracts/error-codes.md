# کدهای خطا — قرارداد پایدار

این سند مرجع رسمی کدهای عددی خطاهای کرنل آریاست. کدها **پایدار** هستند و نباید تغییر کنند؛
منسوخ‌سازی فقط با افزودن واریانت جدید انجام می‌شود.

## بازه‌های اختصاص‌یافته

| بازه | موتور | وضعیت |
|---|---|---|
| 1000–1099 | aria-foundation-engine | فعال |
| 1100–1199 | aria-storage-engine | فعال |
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

### موتور ذخیره‌سازی (1100–1199)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1101 | `db_open_failed` | `error.storage.open_failed` | بازکردن/رمزگشایی پایگاه‌داده ناموفق |
| 1102 | `db_migration_failed` | `error.storage.migration_failed` | مهاجرت نسخه‌دار ناموفق |
| 1103 | `db_query_failed` | `error.storage.query_failed` | خطای پرس‌وجو |
| 1104 | `db_integrity_failed` | `error.storage.integrity_failed` | شکست بررسی صحت |
| 1105 | `db_not_found` | `error.storage.not_found` | موجودیت یافت نشد |
| 1106 | `db_constraint_violation` | `error.storage.constraint_violation` | نقض محدودیت |
| 1107 | `db_transaction_failed` | `error.storage.transaction_failed` | تراکنش ناموفق (rollback شد) |
| 1108 | `db_backup_error` | `error.storage.backup_error` | خطای بکاپ |
| 1109 | `db_attachment_error` | `error.storage.attachment_error` | خطای پیوست |

### موتور امنیت (1200–1299)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1201 | `decryption_failed` | `error.security.decryption_failed` | رمزگشایی/احراز AES-GCM شکست خورد |
| 1202 | `encryption_failed` | `error.security.encryption_failed` | رمزنگاری ناموفق |
| 1203 | `weak_password` | `error.security.weak_password` | نقض سیاست گذرواژه |
| 1204 | `wrong_password` | `error.security.wrong_password` | گذرواژه نادرست (پیام عامدانه کلی) |
| 1205 | `profile_locked` | `error.security.profile_locked` | عملیات نیازمند بازکردن پروفایل |
| 1206 | `vault_not_found` | `error.security.vault_not_found` | گاوصندوق پروفایل مقداردهی نشده |
| 1207 | `vault_already_initialized` | `error.security.vault_already_initialized` | راه‌اندازی مجدد گاوصندوق ممنوع |
| 1208 | `kdf_failed` | `error.security.kdf_failed` | شکست Argon2id یا قالب هش |
| 1209 | `audit_error` | `error.security.audit_error` | خطای ثبت/خواندن حسابرسی |
| 1210 | `malformed_vault_record` | `error.security.malformed_vault_record` | رکورد گاوصندوق خراب/نسخه ناشناخته |

## قواعد

1. هر واریانت خطا باید کد یکتا، واریانت ماشین‌خوان و کلید پیام فارسی داشته باشد.
2. تست «یکتایی کدها» در هر موتور اجباری است.
3. کدها فقط اضافه می‌شوند؛ هرگز بازمقداردهی یا حذف نمی‌شوند.
