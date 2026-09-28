# کدهای خطا — قرارداد پایدار

این سند مرجع رسمی کدهای عددی خطاهای کرنل آریاست. کدها **پایدار** هستند و نباید تغییر کنند؛
منسوخ‌سازی فقط با افزودن واریانت جدید انجام می‌شود.

## بازه‌های اختصاص‌یافته

| بازه | موتور | وضعیت |
|---|---|---|
| 1000–1099 | aria-foundation-engine | فعال |
| 1100–1199 | aria-storage-engine | فعال |
| 1200–1299 | aria-security-engine | فعال |
| 1300–1399 | aria-schema-engine | فعال |
| 1400–1499 | aria-domain-engine | فعال |
| 1500–1599 | aria-plugin-engine | فعال |
| 1600–1699 | aria-runtime-engine | رزرو (فاز ۱.۸) |
| 1700–1799 | aria-query-engine | فعال (فاز ۱.۹) |
| 1800–1899 | aria-ui-engine | فعال (فاز ۱.۱۰) |

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

### موتور فیلدهای سفارشی (1300–1399)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1301 | `field_not_found` | `error.schema.field_not_found` | فیلد یافت نشد |
| 1302 | `duplicate_technical_key` | `error.schema.duplicate_technical_key` | کلید فنی تکراری |
| 1303 | `invalid_field_definition` | `error.schema.invalid_definition` | تعریف فیلد نامعتبر |
| 1304 | `invalid_field_value` | `error.schema.invalid_value` | مقدار نامعتبر برای فیلد |
| 1305 | `field_type_change_forbidden` | `error.schema.type_change_forbidden` | تغییر نوع فیلد دارای داده ممنوع |
| 1306 | `unknown_option_value` | `error.schema.unknown_option` | گزینه ناشناخته برای فیلد select |
| 1307 | `field_has_data` | `error.schema.field_has_data` | عملیات مخرب روی فیلد دارای داده ممنوع |
| 1308 | `schema_storage_error` | `error.schema.storage_error` | خطای ذخیره‌سازی لایه اسکیما |

### موتور دامنه (1400–1499)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1401 | `trade_not_found` | `error.domain.trade_not_found` | معامله یافت نشد (یا حذف نرم شده) |
| 1402 | `account_not_found` | `error.domain.account_not_found` | حساب یافت نشد |
| 1403 | `symbol_not_found` | `error.domain.symbol_not_found` | نماد یافت نشد |
| 1404 | `invalid_trade_data` | `error.domain.invalid_data` | داده معامله/دستور نامعتبر |
| 1405 | `leg_not_found` | `error.domain.leg_not_found` | پا یافت نشد |
| 1406 | `execution_not_found` | `error.domain.execution_not_found` | اجرا یافت نشد |
| 1407 | `assignment_conflict` | `error.domain.assignment_conflict` | تضاد تخصیص اجرا به پا |
| 1408 | `override_error` | `error.domain.override_error` | بازنویسی ناموجود یا بازگشت‌ناپذیر |
| 1409 | `attachment_not_found` | `error.domain.attachment_not_found` | پیوست یافت نشد |
| 1410 | `domain_storage_error` | `error.domain.storage_error` | خطای ذخیره‌سازی لایه دامنه |

### موتور پلاگین (1500–1599)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1501 | `manifest_invalid` | `error.plugin.manifest_invalid` | مانیفست نامعتبر (اسکیما/فیلد الزامی/semver) |
| 1502 | `plugin_not_found` | `error.plugin.not_found` | پلاگین یافت نشد |
| 1503 | `duplicate_plugin` | `error.plugin.duplicate` | شناسه پلاگین از قبل ثبت شده |
| 1504 | `version_incompatible` | `error.plugin.version_incompatible` | ناسازگاری نسخه کرنل یا API با بازه مانیفست |
| 1505 | `unknown_capability` | `error.plugin.unknown_capability` | قابلیت خارج از فهرست بسته |
| 1506 | `permission_denied` | `error.plugin.permission_denied` | فراخوانی بدون قابلیت مجاز یا پلاگین غیرفعال |
| 1507 | `invalid_state_transition` | `error.plugin.invalid_transition` | گذار وضعیت ممنوع در چرخه حیات |
| 1508 | `plugin_quarantined` | `error.plugin.quarantined` | پلاگین در قرنطینه؛ عملیات ممنوع |
| 1509 | `network_access_not_approved` | `error.plugin.network_not_approved` | دسترسی شبکه بدون تأیید صریح کاربر |
| 1510 | `plugin_storage_error` | `error.plugin.storage_error` | خطای ذخیره‌سازی لایه پلاگین |

### موتور پرس‌وجو (1700–1799)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1701 | `invalid_query` | `error.query.invalid_query` | فیلتر، صفحه یا پارامتر پرس‌وجو نامعتبر |
| 1702 | `budget_exceeded` | `error.query.budget_exceeded` | فراتر رفت از بودجه پرس‌وجو (اندازه صفحه > ۲۰۰ یا گام‌های ماشین مجازی) |
| 1703 | `field_not_queryable` | `error.query.field_not_queryable` | فیلد سفارشی ناموجود، غیرفعال یا بدون مجوز فیلتر/آمار |
| 1704 | `query_storage_error` | `error.query.storage_error` | خطای ذخیره‌سازی زیرین موتور پرس‌وجو |

### موتور رابط کاربری (1800–1899)

| کد | واریانت ماشین‌خوان | کلید پیام فارسی | توضیح |
|---|---|---|---|
| 1801 | `invalid_ui_schema` | `error.ui.invalid_schema` | اسکیمای اعلانی نامعتبر (فقدان type رشته‌ای، عنوان خالی و …) |
| 1802 | `duplicate_ui_extension` | `error.ui.duplicate_extension` | شناسه افزونه تکراری در یک نقطه اعلانی |
| 1803 | `ui_extension_not_found` | `error.ui.extension_not_found` | افزونه UI درخواستی یافت نشد |
| 1804 | `ui_injection_denied` | `error.ui.injection_denied` | نوع خارج از فهرست سفید یا کلید تزریق در اسکیما — نسخه ۱ کد اجرایی نمی‌پذیرد |

## قواعد

1. هر واریانت خطا باید کد یکتا، واریانت ماشین‌خوان و کلید پیام فارسی داشته باشد.
2. تست «یکتایی کدها» در هر موتور اجباری است.
3. کدها فقط اضافه می‌شوند؛ هرگز بازمقداردهی یا حذف نمی‌شوند.
