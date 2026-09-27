# قرارداد رویداد — Event Contract (نسخه ۱)

## پاکت رویداد (EventEnvelope)

| فیلد | نوع | الزامی | توضیح |
|---|---|---|---|
| event_id | uuid | بله | شناسه یکتای رویداد |
| event_type | string | بله | مثل `domain.trade_created` |
| event_version | int | بله | نسخه ساختار بار این event_type |
| source | enum | بله | domain_engine، plugin_engine، storage_engine، security_engine، query_engine، runtime_engine یا `plugin:<id>` |
| timestamp | ISO 8601 UTC | بله | زمان صدور |
| correlation_id | uuid | بله | پیوند به دستور عامل |
| payload | object | بله | بار رویداد |

## قواعد

1. رویدادها **پس از commit** منتشر می‌شوند؛ هرگز قبل از آن.
2. رویدادها برای مصرف‌کننده‌ها **idempotent** هستند (event_id برای تشخیص تکرار).
3. رویدادهای حیاتی (دامنه/یکپارچه‌سازی) از **outbox** عبور می‌کنند تا گم نشوند.
4. رویدادهای فقط-UI پایدار نمی‌شوند مگر آنکه نیاز باشد.
5. تغییر ناسازگار بار یک رویداد یعنی event_version جدید (تغییرهای افزودنی در همان نسخه مجازند).

## رویدادهای دامنه (نسخه ۱)

TradeCreated، TradeUpdated، TradeDeleted، EntryLegAdded، ExitLegAdded،
ExecutionAssigned، OverrideAdded، OverrideReverted، StatsInvalidated.

## قاعده پخش داخلی

- مصرف‌کننده‌ی اجباری `StatsInvalidated`: موتور پرس‌وجو → ابطال پروجکشن/کش داشبورد.
- رویدادهای UI با پوشش همزمان (broadcast) به فرانت‌اند می‌رسند؛ ترتیب تضمین‌شده نیست.
