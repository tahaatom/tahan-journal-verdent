# اسکیمای فیزیکی نسخه ۱

منبع حقیقت: `crates/aria-storage-engine/src/schema_v1.rs` (SQL کامل در کد و تست‌شده).

## جدول journal_trades — ستون‌های کلیدی

| ستون | نوع | توضیح |
|---|---|---|
| id | TEXT PK (uuid) | شناسه معامله |
| account_id / symbol_id | TEXT FK | دقیقاً یک حساب و یک نماد |
| direction | buy/sell | جهت |
| status | open/closed/cancelled | وضعیت |
| strategy, timeframe, session, market_condition, entry_type | TEXT | فراداده |
| note, tags, emotions, mistakes | TEXT | یادداشت و برچسب‌ها |
| entry_time / exit_time | TEXT (ISO UTC) | زمان‌ها |
| initial_stop_loss / take_profit | REAL | حد ضرر/سود اولیه |
| manual_risk | REAL | ریسک دستی (اختیاری) |
| risk_calculation_status | pending/calculated/no_stop_loss/manual_risk/needs_assignment | وضعیت محاسبه R |
| risk_basis | initial_stop_loss/manual_risk | مبنای ریسک |
| planned_r, initial_risk_amount, realized_pnl, realized_r, trade_r | REAL | خروجی‌های محاسبه R |
| commission, swap | REAL | هزینه‌ها |
| position_group_id | TEXT | **رزرو نسخه ۱** |
| mae_price, mfe_price, mae_amount, mfe_amount, mae_r, mfe_r | REAL | **رزرو نسخه ۳** |
| max_drawdown_inside_trade | REAL | **رزرو نسخه ۳** |
| deleted_at | TEXT | حذف نرم |

## جدول executions — قواعد

- `leg_id` + `leg_kind`: پای متعلق؛ NULL یعنی بدون پا.
- `assignment_status`: `assigned` یا `needs_assignment` — فقط اجرای assigned در PnL/R.
- `kind`: `manual` یا `imported`؛ `source_record_id` به رکورد خام پیوند می‌خورد.

## ایندکس‌ها

- معاملات: account_id، symbol_id، status، entry_time، ترکیبی (account, entry_time) و (symbol, entry_time)
- پاها: بر اساس trade_id
- اجراها: trade_id، leg_id، assignment_status، ticket
- field_values: field_id و ستون‌های نوع‌دار (text/integer/decimal) با ایندکس جزئی
- پیوست‌ها: blake3_hash؛ پیوندها: trade_id و attachment_id
- حسابرسی: created_at؛ رویدادها: (published, created_at)؛ رکوردهای خام: (batch, status) و hash

## فیلدهای سفارشی

- custom_fields: تعریف کامل (کلید فنی یکتا، نوع ذخیره‌سازی و معنایی، قابل فیلتر/آمار/تحلیل، ترتیب و گروه فرم، قواعد اعتبارسنجی)
- field_options: گزینه‌های enum/multi_enum
- field_values: PK (trade_id, field_id) + ستون‌های نوع‌دار: text_value، integer_value، decimal_value، boolean_value، datetime_value، json_value
