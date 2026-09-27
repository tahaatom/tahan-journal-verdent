# قرارداد دستور — Command Contract (نسخه ۱)

## پاکت دستور (CommandEnvelope)

| فیلد | نوع | الزامی | توضیح |
|---|---|---|---|
| command_id | uuid | بله | شناسه یکتای دستور |
| command_type | string | بله | مثل `domain.create_trade` |
| payload | object | بله | ساختار مخصوص هر command_type |
| issuer | string | بله | `ui` یا `plugin:<id>` یا `kernel:<module>` |
| timestamp | ISO 8601 UTC | بله | زمان صدور |
| correlation_id | uuid | بله | پیوند زنجیره دستور→رویداد |
| envelope_version | int | بله | نسخه قرارداد (فعلاً ۱) |

## قواعد

1. دستورها **اتمیک** اجرا می‌شوند؛ شکست یعنی بدون تغییر.
2. رویدادهای دامنه فقط **پس از commit** با همان correlation_id منتشر می‌شوند.
3. issuer پلاگینی بدون قابلیت لازم، با `PERMISSION_DENIED` رد می‌شود.
4. command_typeها نسخه‌دارند: `domain.<action>`. تغییر ناسازگار payload یعنی نسخه جدید.

## دستورهای دامنه (نسخه ۱)

CreateTradeCommand، UpdateTradeCommand، DeleteTradeCommand، AddEntryLegCommand،
UpdateEntryLegCommand، AddExitLegCommand، UpdateExitLegCommand،
AssignExecutionToLegCommand، AddManualOverrideCommand، RevertManualOverrideCommand،
LinkAttachmentToTradeCommand.

پیاده‌سازی: `aria-domain-engine` — مرجع: `docs/contracts/trade-domain-contract.md`.
