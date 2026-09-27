# قرارداد محاسبه R — R Calculation Contract (نسخه ۱)

R واحد «ریسک پذیرفته‌شده اولیه» است. محاسبه باید **قطعی** و **تست‌شده** باشد.

## خروجی‌های سرویس

| کمیت | تعریف |
|---|---|
| PlannedR | نسبت فاصله ورود تا حد ضرر به اهداف سود (پیش‌نمایش فرم) |
| InitialRiskPriceDistance | فاصله قیمت میانگین ورود تا حد ضرر اولیه |
| InitialRiskAmount | InitialRiskPriceDistance × حجم کل ورود × ارزش واحد |
| RealizedPnL | مجموع سود/زیست محقق‌شده خروج‌ها − کمیسیون − سوآپ |
| RealizedR | RealizedPnL ÷ InitialRiskAmount (یا ریسک دستی) |
| ExitLegR | R هر پای خروج به‌تنهایی (گزارشی؛ مبنای TradeR نیست) |
| TradeR | بر مبنای **کل** نتیجه خالص |
| RiskCalculationStatus | `calculated` / `no_stop_loss` / `manual_risk` / `needs_assignment` |
| RiskBasis | `initial_stop_loss` / `manual_risk` |

## قواعد قطعی

1. ورود چندگانه → قیمت ورود **میانگین وزنی حجم**.
2. مبنای ریسک همیشه **حد ضرر اولیه** است؛ جابجایی حد ضرر بعداً R را تغییر نمی‌دهد.
3. بدون حد ضرر → `no_stop_loss` و **محاسبه خودکار R ممنوع**.
4. کاربر می‌تواند ریسک دستی وارد کند → `manual_risk`.
5. کمیسیون و سوآپ از RealizedPnL **کسر** می‌شوند.
6. خروج چندگانه → جمع PnL خالص همه خروج‌ها ÷ ریسک اولیه = TradeR.
7. Executionهای `needs_assignment` در PnL و R وارد **نمی‌شوند**.
8. MAE/MFE رزرو نسخه ۱ است و در نسخه ۳ با داده مسیر قیمت فعال می‌شود.
