# طرح کلی پایگاه‌داده — Schema (نسخه ۱)

## اصول

1. SQLite رمزنگاری‌شده (سازگار SQLCipher) — کلید از گذرواژه کاربر مشتق می‌شود (موتور امنیت).
2. زمان‌ها همه UTC/ISO 8601 (TEXT)؛ نمایش با جلالی فقط در لایه UI.
3. داده خام (source_records) تغییرناپذیر است؛ ویرایش فقط از مسیر canonical + override ledger.
4. حذف سخت داده کاربر ممنوع — حذف معامله به‌صورت نرم (`deleted_at`) انجام می‌شود.
5. پروجکشن‌ها (trade_daily_summary) کش بازتولیدشدنی‌اند و منبع حقیقت نیستند.

## لایه‌های داده در پایگاه‌داده

| جدول | لایه |
|---|---|
| source_records | Raw Source |
| journal_trades، entry_legs، exit_legs، executions، trading_accounts، symbols، profiles | Canonical |
| manual_overrides | Override Ledger |
| (view زمان خواندن: canonical + overrides) | Effective |
| trade_daily_summary، meta_projection_version | Analytical Projection |

## فهرست جدول‌ها

profiles، trading_accounts، symbols، journal_trades، entry_legs، exit_legs،
executions، source_records، manual_overrides، custom_fields، field_options،
field_values، attachments، attachment_trade_links، audit_logs، system_events
(outbox)، plugins، settings، trade_daily_summary، meta_projection_version.

جزئیات ستون‌ها: `physical-schema-v1.md` — منبع حقیقت: `crates/aria-storage-engine/src/schema_v1.rs`
