//! نگهبان رندر اعلانی — تنها انواع سفید-لیست مجاز به رندر هستند.
//!
//! این ماژول تضمین می‌کند هیچ پلاگینی نمی‌تواند کامپوننت React، اسکریپت
//! یا کد اجرایی وارد درخت UI کند؛ هر چیزی خارج از فهرست سفید → ۱۸۰۴.

use crate::error::UiError;
use crate::registry::ExtensionKind;
use serde_json::Value;

/// انواع ویجت سفید-لیست داشبورد.
pub const DASHBOARD_WIDGET_TYPES: [&str; 7] =
    ["stat", "table", "chart_line", "chart_bar", "text", "divider", "list"];

/// انواع صفحه گزارش سفید-لیست.
pub const REPORT_PAGE_TYPES: [&str; 4] = ["table", "chart_line", "chart_bar", "text"];

/// انواع آیتم منوی فرمان سفید-لیست.
pub const COMMAND_MENU_TYPES: [&str; 1] = ["command"];

/// انواع فیلد فرم سفید-لیست (اعلانی — رندر فرم در کرنل انجام می‌شود).
pub const FORM_FIELD_TYPES: [&str; 6] =
    ["text", "number", "select", "boolean", "date", "textarea"];

/// انواع پنل تنظیمات پلاگین سفید-لیست.
pub const PLUGIN_SETTINGS_TYPES: [&str; 3] = ["text", "number", "boolean"];

/// انواع مجاز برای یک نقطه اعلانی.
pub fn allowed_types(kind: ExtensionKind) -> &'static [&'static str] {
    match kind {
        ExtensionKind::DashboardWidget => &DASHBOARD_WIDGET_TYPES,
        ExtensionKind::ReportPage => &REPORT_PAGE_TYPES,
        ExtensionKind::CommandMenu => &COMMAND_MENU_TYPES,
        ExtensionKind::FormField => &FORM_FIELD_TYPES,
        ExtensionKind::PluginSettings => &PLUGIN_SETTINGS_TYPES,
    }
}

/// کلیدهای ممنوع در اسکیمای اعلانی — هر نشانه‌ای از کد اجرایی/تزریق.
/// شامل هر دو سبک نام‌گذاری (snake_case و camelCase) برای دفاع دوگانه.
const FORBIDDEN_KEYS: [&str; 8] =
    ["component", "jsx", "script", "handler", "on_click", "onClick", "eval", "innerHTML"];

/// اعتبارسنجی اسکیمای اعلانی یک افزونه.
///
/// قواعد:
/// - `type` رشته‌ای و در فهرست سفید نقطه مربوطه باشد (واگرنه ۱۸۰۴).
/// - هیچ کلید ممنوعی در هیچ عمقی از اسکیما نباشد (۱۸۰۴).
/// - `title` اختیاری؛ در صورت وجود باید رشته باشد (۱۸۰۱).
pub fn validate_schema(kind: ExtensionKind, schema: &Value) -> Result<(), UiError> {
    let type_name = schema
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| UiError::InvalidSchema("اسکیمای اعلانی باید «type» رشته‌ای داشته باشد".into()))?;

    let allowed = allowed_types(kind);
    if !allowed.contains(&type_name) {
        return Err(UiError::InjectionDenied(format!(
            "نوع «{type_name}» برای نقطه {} مجاز نیست؛ انواع مجاز: {}",
            kind.as_str(),
            allowed.join(", ")
        )));
    }

    if let Some(title) = schema.get("title") {
        if !title.is_string() {
            return Err(UiError::InvalidSchema("«title» باید رشته باشد".into()));
        }
    }

    reject_forbidden_keys(schema)?;
    Ok(())
}

/// جست‌وجوی بازگشتی کلیدهای ممنوع در همه عمق‌های شیء/آرایه.
fn reject_forbidden_keys(value: &Value) -> Result<(), UiError> {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if FORBIDDEN_KEYS.contains(&k.as_str()) {
                    return Err(UiError::InjectionDenied(format!(
                        "کلید «{k}» در اسکیمای اعلانی ممنوع است (تزریق کد)"
                    )));
                }
                reject_forbidden_keys(v)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for v in items {
                reject_forbidden_keys(v)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_widgets_pass() {
        for t in DASHBOARD_WIDGET_TYPES {
            let schema = json!({"type": t, "title": "عنوان"});
            assert!(
                validate_schema(ExtensionKind::DashboardWidget, &schema).is_ok(),
                "type={t}"
            );
        }
    }

    #[test]
    fn type_scoped_to_extension_point() {
        // stat فقط برای dashboard_widget مجاز است
        assert!(validate_schema(ExtensionKind::DashboardWidget, &json!({"type": "stat"})).is_ok());
        assert_eq!(
            validate_schema(ExtensionKind::ReportPage, &json!({"type": "stat"}))
                .unwrap_err()
                .code(),
            1804
        );
        // command فقط برای command_menu
        assert!(validate_schema(ExtensionKind::CommandMenu, &json!({"type": "command"})).is_ok());
        assert_eq!(
            validate_schema(ExtensionKind::FormField, &json!({"type": "command"}))
                .unwrap_err()
                .code(),
            1804
        );
    }

    #[test]
    fn missing_or_non_string_type_is_1801() {
        assert_eq!(
            validate_schema(ExtensionKind::DashboardWidget, &json!({}))
                .unwrap_err()
                .code(),
            1801
        );
        assert_eq!(
            validate_schema(ExtensionKind::DashboardWidget, &json!({"type": 42}))
                .unwrap_err()
                .code(),
            1801
        );
    }

    #[test]
    fn injection_keys_denied_at_any_depth_1804() {
        let cases = vec![
            json!({"type": "table", "component": "StealData"}),
            json!({"type": "table", "rows": [{"jsx": "<script/>"}]}),
            json!({"type": "list", "items": {"handler": "fetch"}}),
            json!({"type": "text", "meta": {"on_click": "evil"}}),
            json!({"type": "text", "deep": [{"script": "x"}]}),
            json!({"type": "text", "onClick": "evil"}),
            json!({"type": "list", "items": [{"innerHTML": "<img/>"}]}),
        ];
        for schema in cases {
            let err = validate_schema(ExtensionKind::DashboardWidget, &schema).unwrap_err();
            assert_eq!(err.code(), 1804, "schema={schema}");
        }
    }

    #[test]
    fn non_string_title_is_1801() {
        assert_eq!(
            validate_schema(
                ExtensionKind::DashboardWidget,
                &json!({"type": "stat", "title": 5})
            )
            .unwrap_err()
            .code(),
            1801
        );
    }
}
