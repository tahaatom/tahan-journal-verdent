//! رجیستری افزونه‌پذیری UI — فقط نقاط اعلانی (نسخه ۱).
//!
//! پلاگین‌ها اسکیمای اعلانی ثبت می‌کنند؛ هیچ کامپوننت React یا کد اجرایی
//! از این مسیر عبور نمی‌کند (نگهبان رندر در `render.rs`).

use crate::error::UiError;
use crate::render::validate_schema;
use aria_plugin_engine::manifest::{UiExtensionPoint, UI_EXTENSION_KINDS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// نوع نقطه اعلانی UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    DashboardWidget,
    ReportPage,
    CommandMenu,
    FormField,
    PluginSettings,
}

impl ExtensionKind {
    /// تجزیه از رشته مانیفست.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "dashboard_widget" => Self::DashboardWidget,
            "report_page" => Self::ReportPage,
            "command_menu" => Self::CommandMenu,
            "form_field" => Self::FormField,
            "plugin_settings" => Self::PluginSettings,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DashboardWidget => "dashboard_widget",
            Self::ReportPage => "report_page",
            Self::CommandMenu => "command_menu",
            Self::FormField => "form_field",
            Self::PluginSettings => "plugin_settings",
        }
    }

    /// همه انواع مجاز — هم‌ارز ثابت مانیفست.
    pub fn all() -> &'static [&'static str] {
        &UI_EXTENSION_KINDS
    }
}

/// یک افزونه UI اعلانی ثبت‌شده.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiExtension {
    /// شناسه پلاگین مالک افزونه
    pub plugin_id: String,
    pub kind: ExtensionKind,
    /// شناسه یکتا در ترکیب با kind
    pub id: String,
    /// برچسب فارسی نمایشی
    pub title: String,
    /// ترتیب نمایش (کوچک‌تر = جلوتر)
    pub display_order: i64,
    /// اسکیمای اعلانی رندر — فقط انواع سفید-لیست رندر (`render.rs`)
    pub schema: serde_json::Value,
}

/// رجیستری افزونه‌های UI اعلانی.
///
/// کلید یکتایی: `(kind, id)`. افزونه‌های متفاوت نمی‌توانند شناسه تکراری
/// در یک نقطه داشته باشند تا رندر قطعی و بدون تداخل بماند.
#[derive(Debug, Default)]
pub struct UiExtensionRegistry {
    extensions: BTreeMap<(ExtensionKind, String), UiExtension>,
}

impl UiExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// تعداد افزونه‌های ثبت‌شده.
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }

    /// ثبت یک افزونه اعلانی با اعتبارسنجی کامل.
    pub fn register(&mut self, ext: UiExtension) -> Result<(), UiError> {
        if ext.plugin_id.trim().is_empty() {
            return Err(UiError::InvalidSchema("شناسه پلاگین خالی است".into()));
        }
        if ext.id.trim().is_empty() {
            return Err(UiError::InvalidSchema("شناسه افزونه خالی است".into()));
        }
        if ext.title.trim().is_empty() {
            return Err(UiError::InvalidSchema("برچسب افزونه خالی است".into()));
        }
        if !ext.schema.is_object() {
            return Err(UiError::InvalidSchema(format!(
                "اسکیمای افزونه «{}» باید شیء اعلانی باشد",
                ext.id
            )));
        }
        validate_schema(ext.kind, &ext.schema)?;
        let key = (ext.kind, ext.id.clone());
        if self.extensions.contains_key(&key) {
            return Err(UiError::DuplicateExtension(format!(
                "{:?}/{}",
                key.0, key.1
            )));
        }
        tracing::debug!(
            plugin = %ext.plugin_id,
            kind = ext.kind.as_str(),
            id = %ext.id,
            "افزونه UI ثبت شد"
        );
        self.extensions.insert(key, ext);
        Ok(())
    }

    /// ثبت مستقیم از نقطه اعلانی مانیفست پلاگین.
    pub fn register_from_manifest(
        &mut self,
        plugin_id: &str,
        point: &UiExtensionPoint,
    ) -> Result<(), UiError> {
        let kind = ExtensionKind::parse(&point.kind).ok_or_else(|| {
            UiError::InvalidSchema(format!("نوع نقطه اعلانی نامعتبر: {}", point.kind))
        })?;
        self.register(UiExtension {
            plugin_id: plugin_id.to_string(),
            kind,
            id: point.id.clone(),
            title: point
                .schema
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(&point.id)
                .to_string(),
            display_order: point
                .schema
                .get("order")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            schema: point.schema.clone(),
        })
    }

    /// حذف همه افزونه‌های یک پلاگین (هنگام غیرفعال‌سازی/حذف پلاگین).
    /// خروجی: تعداد افزونه‌های حذف‌شده.
    pub fn unregister_plugin(&mut self, plugin_id: &str) -> usize {
        let before = self.extensions.len();
        self.extensions
            .retain(|_, e| e.plugin_id != plugin_id);
        let removed = before - self.extensions.len();
        if removed > 0 {
            tracing::debug!(plugin = %plugin_id, removed, "افزونه‌های پلاگین حذف شدند");
        }
        removed
    }

    /// فهرست افزونه‌های یک نقطه، مرتب‌شده بر اساس (order, id).
    pub fn list(&self, kind: ExtensionKind) -> Vec<&UiExtension> {
        let mut out: Vec<&UiExtension> = self
            .extensions
            .values()
            .filter(|e| e.kind == kind)
            .collect();
        out.sort_by(|a, b| {
            (a.display_order, &a.id).cmp(&(b.display_order, &b.id))
        });
        out
    }

    /// بازیابی یک افزونه مشخص.
    pub fn get(&self, kind: ExtensionKind, id: &str) -> Option<&UiExtension> {
        self.extensions.get(&(kind, id.to_string()))
    }

    /// فهرست افزونه‌های یک پلاگین.
    pub fn list_of_plugin(&self, plugin_id: &str) -> Vec<&UiExtension> {
        self.extensions
            .values()
            .filter(|e| e.plugin_id == plugin_id)
            .collect()
    }

    /// سریال‌سازی کامل برای عرضه به فرانت‌اند (بدون هیچ کد اجرایی).
    pub fn snapshot(&self, kind: ExtensionKind) -> Vec<serde_json::Value> {
        self.list(kind)
            .iter()
            .map(|e| {
                serde_json::json!({
                    "plugin_id": e.plugin_id,
                    "kind": e.kind.as_str(),
                    "id": e.id,
                    "title": e.title,
                    "display_order": e.display_order,
                    "schema": e.schema,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn widget(id: &str, widget_type: &str) -> UiExtension {
        UiExtension {
            plugin_id: "plg-test".into(),
            kind: ExtensionKind::DashboardWidget,
            id: id.into(),
            title: format!("ویجت {id}"),
            display_order: 0,
            schema: json!({"type": widget_type, "title": "عنوان"}),
        }
    }

    #[test]
    fn kinds_match_manifest_constants() {
        assert_eq!(
            ExtensionKind::all(),
            &[
                "dashboard_widget",
                "report_page",
                "command_menu",
                "form_field",
                "plugin_settings"
            ]
        );
        for s in ExtensionKind::all() {
            assert!(ExtensionKind::parse(s).is_some());
        }
        assert!(ExtensionKind::parse("react_component").is_none());
    }

    #[test]
    fn register_and_list_ordered() {
        let mut reg = UiExtensionRegistry::new();
        let mut b = widget("b", "stat");
        b.display_order = 2;
        let mut a = widget("a", "stat");
        a.display_order = 1;
        reg.register(b).unwrap();
        reg.register(a).unwrap();
        let list = reg.list(ExtensionKind::DashboardWidget);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "a");
        assert_eq!(list[1].id, "b");
    }

    #[test]
    fn duplicate_id_in_same_kind_rejected_1802() {
        let mut reg = UiExtensionRegistry::new();
        reg.register(widget("w1", "stat")).unwrap();
        let err = reg.register(widget("w1", "stat")).unwrap_err();
        assert_eq!(err.code(), 1802);
        // همان شناسه در نقطه دیگر مجاز است (نوع مجاز همان نقطه)
        let mut page = widget("w1", "table");
        page.kind = ExtensionKind::ReportPage;
        assert!(reg.register(page).is_ok());
    }

    #[test]
    fn invalid_fields_rejected_1801() {
        let mut reg = UiExtensionRegistry::new();
        let mut e = widget("w", "stat");
        e.plugin_id = "  ".into();
        assert_eq!(reg.register(e).unwrap_err().code(), 1801);
        let mut e = widget("w", "stat");
        e.title = String::new();
        assert_eq!(reg.register(e).unwrap_err().code(), 1801);
        let mut e = widget("w", "stat");
        e.schema = json!("raw");
        assert_eq!(reg.register(e).unwrap_err().code(), 1801);
    }

    #[test]
    fn unknown_widget_type_is_injection_denied_1804() {
        let mut reg = UiExtensionRegistry::new();
        let e = widget("evil", "react_component");
        let err = reg.register(e).unwrap_err();
        assert_eq!(err.code(), 1804);
        assert!(reg.is_empty());
    }

    #[test]
    fn unregister_plugin_removes_only_its_extensions() {
        let mut reg = UiExtensionRegistry::new();
        let mut mine = widget("mine", "stat");
        mine.plugin_id = "plg-a".into();
        let mut other = widget("other", "stat");
        other.plugin_id = "plg-b".into();
        reg.register(mine).unwrap();
        reg.register(other).unwrap();
        assert_eq!(reg.unregister_plugin("plg-a"), 1);
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.unregister_plugin("plg-a"), 0);
        assert!(reg.get(ExtensionKind::DashboardWidget, "other").is_some());
    }

    #[test]
    fn register_from_manifest_maps_title_and_order() {
        let mut reg = UiExtensionRegistry::new();
        let point = UiExtensionPoint {
            kind: "dashboard_widget".into(),
            id: "w1".into(),
            schema: json!({"type": "stat", "title": "سود امروز", "order": 5}),
        };
        reg.register_from_manifest("plg-x", &point).unwrap();
        let e = reg.get(ExtensionKind::DashboardWidget, "w1").unwrap();
        assert_eq!(e.title, "سود امروز");
        assert_eq!(e.display_order, 5);
        assert_eq!(e.plugin_id, "plg-x");
    }

    #[test]
    fn snapshot_is_pure_json() {
        let mut reg = UiExtensionRegistry::new();
        reg.register(widget("w1", "stat")).unwrap();
        let snap = reg.snapshot(ExtensionKind::DashboardWidget);
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0]["kind"], "dashboard_widget");
        assert_eq!(snap[0]["schema"]["type"], "stat");
    }
}
