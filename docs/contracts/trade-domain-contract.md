# قرارداد دامنه معاملاتی — Trade Domain Contract (نسخه ۱)

## موجودیت‌ها

- **Profile**: پروفایل کاربری (داده هر پروفایل جدا و رمزنگاری‌شده است).
- **TradingAccount**: حساب معاملاتی (بروکر، نوع، ارز پایه).
- **Symbol**: نماد معاملاتی.
- **JournalTrade**: ژورنال معامله — واحد تصمیم مستقل.
- **EntryLeg / ExitLeg**: پاهای معنایی ورود/خروج.
- **Execution**: فیل واقعی بروکر (یا دستی) — همیشه متعلق به دقیقاً یک پا.
- **SourceRecord**: رکورد خام واردشده — **غیرقابل تغییر**.
- **ManualOverride**: بازنویسی دستی با تاریخچه کامل.
- **PositionGroup**: در نسخه ۱ رزرو — هر معامله دقیقاً یک گروه ضمنی دارد.

## قواعد نسخه ۱

1. هر JournalTrade دقیقاً یک حساب، یک نماد و یک جهت (buy/sell) دارد.
2. هر JournalTrade یک گروه پوزیشن ضمنی دارد؛ تصمیم‌های مستقل = معامله‌های جدا.
3. ورود/خروج چندگانه فقط از طریق پاها.
4. معامله دستی → Execution دستی می‌سازد.
5. داده واردشده → اول SourceRecord، بعد Execution، بعد تخصیص به پا.
6. Execution بدون پا → `needs_assignment` و **خارج از PnL و R**.
7. حذف دستوری معامله با تأیید و ثبت در حسابرسی؛ داده خام هرگز حذف نمی‌شود.

## دستورها و رویدادها

- دستورها: CreateTrade، UpdateTrade، DeleteTrade، AddEntryLeg، UpdateEntryLeg،
  AddExitLeg، UpdateExitLeg، AssignExecutionToLeg، AddManualOverride،
  RevertManualOverride، LinkAttachmentToTrade.
- رویدادها: TradeCreated، TradeUpdated، TradeDeleted، EntryLegAdded، ExitLegAdded،
  ExecutionAssigned، OverrideAdded، OverrideReverted، StatsInvalidated.

## لایه‌های داده

| لایه | ویژگی |
|---|---|
| Raw Source | تغییرناپذیر، دقیقاً بار منبع، ردیابی هش |
| Canonical | داده ژورنال نرمال، ویرایش فقط با دستورهای رسمی |
| Override Ledger | مقدار قبلی/جدید، دلیل، زمان، منبع، اولویت، بازگشت‌پذیری |
| Effective | خروجی نهایی برای نمایش/فیلتر/آمار = canonical + overrides |
| Analytical Projection | جدول‌های خلاصه و کش — بازتولیدشدنی، منبع حقیقت نیست |

## پیاده‌سازی (فاز ۱.۶)

- کرت `aria-domain-engine`: `model.rs` (مدل)، `commands.rs` (۱۱ دستور)، `events.rs` (کارخانه رویداد + outbox)،
  `risk.rs` (محاسبه R قطعی)، `service.rs` (اجرای اتمیک دستورها، داده مؤثر، بازمحاسبه)، `error.rs` (کدهای ۱۴۰۱–۱۴۱۰).
- تصمیم‌های تکمیلی: فرضیات A-011 تا A-014 در `docs/ASSUMPTIONS.md`.
