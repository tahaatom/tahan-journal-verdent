# API پلاگین — Plugin API (نسخه ۱)

## اصول

1. پلاگین‌ها **هیچ** دسترسی مستقیمی به پایگاه‌داده، حافظه کرنل، فایل‌سیستم یا سایر پلاگین‌ها ندارند.
2. تنها کانال ارتباط: **JSON-RPC 2.0 روی stdio** با پیام‌های newline-delimited.
3. محدودیت پیام: 10 MB — مهلت پیش‌فرض: 30 ثانیه — احراز هویت با توکن نشست.
4. `network.access` به‌صورت پیش‌فرض خاموش است و فقط با تأیید صریح کاربر فعال می‌شود.

## چرخه حیات

`Install → validate manifest → check permissions → user approval → start → monitor → crash handling → quarantine/disable`

- بیشتر از ۳ کرش متوالی → **قرنطینه** (فعال‌سازی مجدد فقط دستی).
- کرش پلاگین هرگز کرنل را از کار نمی‌اندازد.

## متدهای سرویس‌پذیر به پلاگین (نسخه ۱)

| متد | قابلیت لازم | توضیح |
|---|---|---|
| trades.list | trades.read | فهرست صفحه‌بندی‌شده معاملات |
| trades.get | trades.read | معامله با داده مؤثر |
| trades.create | trades.write | ثبت معامله (دستور دامنه) |
| fields.list | fields.read | فراداده فیلدهای سفارشی |
| stats.dashboard | stats.read | خلاصه داشبورد از پروجکشن |
| insights.write | insights.write | ثبت بینش با شواهد |
| notifications.show | notifications.show | اعلان به UI |

متدهای نامعلوم با `METHOD_NOT_FOUND` رد می‌شوند؛ فراخوانی بدون قابلیت با `PERMISSION_DENIED`.

## افزونه‌پذیری UI (نسخه ۱)

- فقط نقاط اعلانی: `dashboard_widget`، `report_page`، `command_menu`، `form_field`، `plugin_settings`.
- کرنل اسکیمای اعلانی را رندر می‌کند؛ **تزریق مستقیم کامپوننت React ممنوع**.

## پیاده‌سازی (فاز ۱.۷)

- کرت `aria-plugin-engine`: `manifest.rs` (اعتبارسنجی مانیفست)، `semver.rs` (نسخه‌گذاری معنایی + بازه‌های سازگاری)،
  `permissions.rs` (۱۷ قابلیت فهرست بسته)، `service.rs` (رجیستری، چرخه حیات، قرنطینه، اجرای مجوز، سلامت)،
  `events.rs` (رویدادهای plugin.* با outbox)، `error.rs` (کدهای ۱۵۰۱–۱۵۱۰).
- محدوده نسخه ۱: بدون اجرای فرایند جانبی — راه‌اندازی real-time و ناظر فرایند در فاز ۱.۸ (موتور اجرا) پیاده می‌شود؛
  `start/stop` در این فاز فقط وضعیت رجیستری و رویدادها را مدیریت می‌کنند و `runtime_limits` قرارداد گذار به موتور اجراست.
- تصمیم‌های تکمیلی: فرضیات A-015 تا A-019 در `docs/ASSUMPTIONS.md`.
