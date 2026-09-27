# خط پایه فناوری — Tahan Journal / Aria Kernel

نسخه سند: 1 — فاز ۰

## نسخه‌های دقیق یا مجاز

| مؤلفه | نسخه مجاز | ملاحظات |
|---|---|---|
| Rust | stable (تثبیت‌شده با rust-toolchain.toml) | کامپوننت‌های rustfmt و clippy اجباری |
| Rust Edition | 2021 | |
| Tauri | 2.x | فقط Tauri 2 |
| Node.js | 22 LTS یا بالاتر | برای توسعه فرانت‌اند |
| Python | 3.11+ (فقط برای سایدکار) | venv جداگانه برای هر پلاگین |
| React | 18+ | |
| TypeScript | strict mode | `strict: true` اجباری |
| Vite | 5+ | |
| SQLite (rusqlite bundled) | 0.31+ | |

## کتابخانه‌های تأییدشده (Backend/Rust)

serde, serde_json, thiserror, anyhow, uuid, chrono, tracing, tracing-subscriber,
tracing-appender, rusqlite, semver, jsonschema, sysinfo, argon2, aes-gcm, zeroize,
secrecy, blake3, zstd, image, csv, encoding_rs, directories/dirs, tokio, refinery
(یا مهاجرت مبتنی بر PRAGMA user_version), تقویم جلالی سازگار با chrono.

## کتابخانه‌های تأییدشده (Frontend)

React 18+, TypeScript strict, Vite, Tailwind CSS, Radix UI یا Headless UI,
React Hook Form, Zod, @hookform/resolvers, TanStack Table, TanStack Virtual,
TanStack Query, Zustand یا Jotai, Apache ECharts, i18next, react-i18next,
date-fns-jalali یا پلاگین جلالی dayjs, فونت Vazirmatn, Vitest,
React Testing Library, Playwright یا tauri-driver.

## کتابخانه‌های تأییدشده (Python Sidecar)

- نسخه ۱ و ۲: polars, pandas, numpy, scipy, matplotlib
- گزارش‌های پیشرفته نسخه ۲: fpdf2, arabic_reshaper, python-bidi
- وابستگی‌ها از پیش بسته‌بندی می‌شوند؛ نصب اینترنتی در نسخه ۱ ممنوع است.

## کتابخانه‌های ممنوع

- هر کتابخانه‌ای که دسترسی شبکه بدون مجوز صریح کاربر ایجاد کند.
- ORMهای سنگین روی SQLite (منبع حقیقت، SQL شفاف است).
- کتابخانه‌های اجرای کد دلخواه در کرنل (مثلاً مفسرهای پویا داخل کرنل).
- هر وابستگی سنگین جدید بدون تأیید صریح (طبق قوانین پرامپت مادر).

## سیاست ارتقاء

1. ارتقاء نسخه‌های فرعی (patch/minor) کتابخانه‌های تأییدشده آزاد است، مشروط به سبز بودن کل تست‌ها.
2. ارتقاء major هر کتابخانه یا Tauri/React باید به‌صورت یک فاز مستقر ثبت شود، با رفع سازگاری قراردادها.
3. تغییر قراردادهای عمومی `aria-contracts` بدون سیاست منسوخ‌سازی صریح ممنوع است.
4. هر تغییر فناوری باید در همین سند و در docs/ASSUMPTIONS.md ثبت شود.

## سیاست مهاجرت تغییرات فناوری آینده

- موتورها فقط از طریق قراردادهای نسخه‌دار با هم صحبت می‌کنند؛ بنابراین تعویض پیاده‌سازی داخلی
  (مثلاً جابجایی IPC یا موتور ذخیره‌سازی) نباید لایه‌های بالاتر را بشکند.
- اسکیمای پایگاه‌داده فقط با مهاجرت‌های نسخه‌دار و بدون از دست رفتن داده تغییر می‌کند.
- فرمت بکاپ نسخه‌دار است؛ بکاپ‌های قدیمی همیشه باید قابل بازیابی بمانند.
