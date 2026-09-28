# مدل فیلدهای سفارشی — شهروند درجه‌یک تحلیلی

فیلدهای سفارشی در کرنل آریا **اعلانی** هستند (بدون کد): کاربر تعریف می‌کند، کرنل در فرم، فیلتر، لیست،
داشبورد و آمار رفتار یکسان و نوع‌دار ارائه می‌دهد. موتور مربوطه: `aria-schema-engine` (بازه خطا ۱۳۰۰–۱۳۹۹).

## اجزای موتور

| ماژول | مسئولیت |
|---|---|
| `model.rs` | `StorageType` (۱۰ نوع)، `SemanticType` (۱۲ نوع)، `ValidationRules`، `FieldDefinition`، اعتبارسنجی ساختاری |
| `validation.rs` | اعتبارسنجی مقادیر نوع‌دار: بازه عددی، طول متن، ISO 8601 برای datetime، گزینه‌های فعال، بدون تکرار برای multi_enum |
| `service.rs` | CRUD فیلدها/گزینه‌ها/مقادیر روی پایگاه‌داده + `form_metadata` (گروه‌بندی فرم) + `filterable_fields` |

## نوع ذخیره‌سازی و ستون مقصد

مقادیر در جدول `field_values` با ستون‌های نوع‌دار ذخیره می‌شوند (کلید: `trade_id + field_id`):

| StorageType | ستون | نوع معنایی مرتبط |
|---|---|---|
| `text` / `long_text` / `enum` | `text_value` | ShortText / LongText / SingleSelect |
| `multi_enum` / `tag_set` | `json_value` (آرایه رشته) | MultiSelect / Tag |
| `integer` / `rating` | `integer_value` | Number / Rating (۱ تا ۵) |
| `decimal` | `decimal_value` | Number / Price / Percent / Money |
| `boolean` | `boolean_value` | Boolean |
| `datetime` | `datetime_value` (ISO 8601 UTC) | Datetime |

## قواعد پایدار نسخه ۱

1. **تغییر `storage_type`** فقط تا وقتی فیلد داده ندارد مجاز است؛ در صورت وجود داده خطا ۱۳۰۵.
2. **حذف سخت** فیلد دارای داده ممنوع — فقط غیرفعال‌سازی (`active = 0`)؛ داده‌ها حفظ می‌شوند.
3. **گزینه‌های enum فقط افزودنی** هستند؛ گزینه استفاده‌شده فقط غیرفعال می‌شود و مقدار قدیمی معتبر می‌ماند.
4. مقدار enum/multi_enum/tag_set باید از **گزینه‌های فعال** باشد (خطا ۱۳۰۶)؛ multi_enum تکرار نمی‌پذیرد.
5. `rating` فقط عدد صحیح ۱ تا ۵؛ `decimal` باید محدود (finite) باشد؛ بازه‌ها از `ValidationRules` می‌آیند.
6. کلید فنی (`technical_key`) یکتا و در الگوی `snake_case` (خطا ۱۳۰۲/۱۳۰۳).
7. ذخیره/بازخوانی مقادیر فقط از مجرای `validate_value` انجام می‌شود — اعتبارسنجی پیش از هر نوشتن.

## مصرف در لایه‌های بالاتر (قرارداد)

- **فرم**: `form_metadata()` خروجی JSON گروه‌بندی‌شده با ویجت استنتاج‌شده از `semantic_type` می‌دهد (بدون کد سمت فرانت).
- **فیلتر و لیست**: فقط فیلدهای `filterable = true` در فیلترها و ستون‌های لیست ظاهر می‌شوند.
- **آمار و تحلیل**: `stat_enabled` برای آمار و `analysis_enabled` برای تحلیل پیشرفته.

## کدهای خطا

| کد | واریانت | توضیح |
|---|---|---|
| ۱۳۰۱ | `field_not_found` | فیلد یافت نشد |
| ۱۳۰۲ | `duplicate_technical_key` | کلید فنی تکراری |
| ۱۳۰۳ | `invalid_field_definition` | تعریف نامعتبر |
| ۱۳۰۴ | `invalid_field_value` | مقدار نامعتبر |
| ۱۳۰۵ | `field_type_change_forbidden` | تغییر نوع فیلد دارای داده |
| ۱۳۰۶ | `unknown_option_value` | گزینه ناشناخته |
| ۱۳۰۷ | `field_has_data` | عملیات مخرب روی فیلد دارای داده |
| ۱۳۰۸ | `schema_storage_error` | خطای ذخیره‌سازی اسکیما |
