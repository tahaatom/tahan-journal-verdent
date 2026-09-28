//! سرویس اسکیما — CRUD فیلدها و مقادیر، فراداده فرم و پل به قرارداد عمومی.

use crate::error::SchemaError;
use crate::model::{validate_definition, FieldDefinition, FieldOption, SemanticType, StorageType};
use crate::validation::{validate_value, TypedValue};
use aria_storage_engine::Database;

/// سرویس فیلدهای سفارشی روی یک پایگاه‌داده باز.
pub struct SchemaService<'a> {
    db: &'a Database,
}

impl<'a> SchemaService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    // ================= تعریف فیلدها =================

    /// تعریف فیلد جدید — کلید فنی یکتا و ساختار معتبر الزامی است.
    pub fn define_field(&self, mut def: FieldDefinition) -> Result<FieldDefinition, SchemaError> {
        validate_definition(&def)?;
        def.schema_version = def.schema_version.max(1);
        let conn = self.db.lock();
        let exists: i64 = conn.query_row(
            "SELECT count(*) FROM custom_fields WHERE technical_key = ?1",
            [&def.technical_key],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Err(SchemaError::duplicate_key(def.technical_key.clone()));
        }
        conn.execute(
            "INSERT INTO custom_fields (id, technical_key, display_label, description, storage_type, semantic_type,
             unit, default_value, required, active, filterable, stat_enabled, analysis_enabled,
             display_order, form_group, validation_rules, schema_version, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            rusqlite::params![
                def.id,
                def.technical_key,
                def.display_label,
                def.description,
                def.storage_type.as_str(),
                def.semantic_type.as_str(),
                def.unit,
                def.default_value.as_ref().map(|v| v.to_string()),
                def.required as i64,
                def.active as i64,
                def.filterable as i64,
                def.stat_enabled as i64,
                def.analysis_enabled as i64,
                def.display_order,
                def.form_group,
                serde_json::to_string(&def.validation_rules).map_err(|e| SchemaError::storage(e.to_string()))?,
                def.schema_version as i64,
                now_iso(),
                now_iso(),
            ],
        )?;
        tracing::info!(field = %def.technical_key, "custom field defined");
        Ok(def)
    }

    /// افزودن گزینه به فیلد select.
    pub fn add_option(&self, field_id: &str, value: &str, label: &str, sort_order: i64) -> Result<FieldOption, SchemaError> {
        if value.trim().is_empty() || label.trim().is_empty() {
            return Err(SchemaError::invalid_definition("option value/label are empty"));
        }
        let field = self.get_field(field_id)?;
        if !field.storage_type.needs_options() {
            return Err(SchemaError::invalid_definition(format!(
                "field {} does not accept options",
                field.storage_type.value_column()
            )));
        }
        let conn = self.db.lock();
        let dup: i64 = conn.query_row(
            "SELECT count(*) FROM field_options WHERE field_id = ?1 AND value = ?2",
            rusqlite::params![field_id, value],
            |r| r.get(0),
        )?;
        if dup > 0 {
            return Err(SchemaError::invalid_definition(format!("duplicate option value {value}")));
        }
        let opt = FieldOption {
            id: uuid::Uuid::new_v4().to_string(),
            field_id: field_id.to_string(),
            value: value.to_string(),
            label: label.to_string(),
            sort_order,
            active: true,
        };
        conn.execute(
            "INSERT INTO field_options (id, field_id, value, label, sort_order, active) VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![opt.id, opt.field_id, opt.value, opt.label, opt.sort_order, opt.active as i64],
        )?;
        Ok(opt)
    }

    /// خواندن تعریف فیلد.
    pub fn get_field(&self, field_id: &str) -> Result<FieldDefinition, SchemaError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, technical_key, display_label, description, storage_type, semantic_type, unit,
             default_value, required, active, filterable, stat_enabled, analysis_enabled,
             display_order, form_group, validation_rules, schema_version
             FROM custom_fields WHERE id = ?1",
        )?;
        let mut rows = stmt.query([field_id])?;
        match rows.next()? {
            Some(r) => Ok(map_field_row(r)?),
            None => Err(SchemaError::field_not_found(field_id)),
        }
    }

    /// یافتن فیلد با کلید فنی.
    pub fn find_by_key(&self, technical_key: &str) -> Result<Option<FieldDefinition>, SchemaError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, technical_key, display_label, description, storage_type, semantic_type, unit,
             default_value, required, active, filterable, stat_enabled, analysis_enabled,
             display_order, form_group, validation_rules, schema_version
             FROM custom_fields WHERE technical_key = ?1",
        )?;
        let mut rows = stmt.query([technical_key])?;
        match rows.next()? {
            Some(r) => Ok(Some(map_field_row(r)?)),
            None => Ok(None),
        }
    }

    /// فهرست فیلدها (اختیاری: همراه غیرفعال‌ها) مرتب بر اساس گروه و ترتیب نمایش.
    pub fn list_fields(&self, include_inactive: bool) -> Result<Vec<FieldDefinition>, SchemaError> {
        let conn = self.db.lock();
        let sql = if include_inactive {
            "SELECT id, technical_key, display_label, description, storage_type, semantic_type, unit,
             default_value, required, active, filterable, stat_enabled, analysis_enabled,
             display_order, form_group, validation_rules, schema_version
             FROM custom_fields ORDER BY form_group, display_order, technical_key"
        } else {
            "SELECT id, technical_key, display_label, description, storage_type, semantic_type, unit,
             default_value, required, active, filterable, stat_enabled, analysis_enabled,
             display_order, form_group, validation_rules, schema_version
             FROM custom_fields WHERE active = 1 ORDER BY form_group, display_order, technical_key"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], map_field_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// فیلدهای قابل فیلتر (برای فیلترها و ستون‌های لیست).
    pub fn filterable_fields(&self) -> Result<Vec<FieldDefinition>, SchemaError> {
        Ok(self
            .list_fields(false)?
            .into_iter()
            .filter(|f| f.filterable)
            .collect())
    }

    /// فراداده فرم — گروه‌بندی اعلانی برای رندر (بدون کد).
    pub fn form_metadata(&self) -> Result<serde_json::Value, SchemaError> {
        let fields = self.list_fields(false)?;
        let mut groups: Vec<(String, Vec<&FieldDefinition>)> = Vec::new();
        for f in &fields {
            let group = f.form_group.clone().unwrap_or_else(|| "عمومی".to_string());
            match groups.iter_mut().find(|(g, _)| *g == group) {
                Some((_, items)) => items.push(f),
                None => groups.push((group, vec![f])),
            }
        }
        let group_json: Vec<serde_json::Value> = groups
            .into_iter()
            .map(|(name, items)| {
                serde_json::json!({
                    "group": name,
                    "fields": items.iter().map(|f| field_form_json(f)).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "groups": group_json }))
    }

    // ================= تغییر وضعیت فیلدها =================

    /// به‌روزرسانی کنترل‌شده فیلد — تغییر نوع فقط برای فیلد بدون داده.
    pub fn update_field(
        &self,
        field_id: &str,
        new_def: FieldDefinition,
    ) -> Result<FieldDefinition, SchemaError> {
        validate_definition(&new_def)?;
        let current = self.get_field(field_id)?;
        if new_def.storage_type != current.storage_type && self.field_has_data(field_id)? {
            return Err(SchemaError::type_change_forbidden(field_id));
        }
        // کلید فنی نباید با فیلد دیگری تداخل کند
        if new_def.technical_key != current.technical_key {
            if let Some(other) = self.find_by_key(&new_def.technical_key)? {
                if other.id != field_id {
                    return Err(SchemaError::duplicate_key(new_def.technical_key.clone()));
                }
            }
        }
        self.db.lock().execute(
            "UPDATE custom_fields SET technical_key=?2, display_label=?3, description=?4, storage_type=?5,
             semantic_type=?6, unit=?7, default_value=?8, required=?9, active=?10, filterable=?11,
             stat_enabled=?12, analysis_enabled=?13, display_order=?14, form_group=?15,
             validation_rules=?16, schema_version=?17, updated_at=?18
             WHERE id=?1",
            rusqlite::params![
                field_id,
                new_def.technical_key,
                new_def.display_label,
                new_def.description,
                new_def.storage_type.as_str(),
                new_def.semantic_type.as_str(),
                new_def.unit,
                new_def.default_value.as_ref().map(|v| v.to_string()),
                new_def.required as i64,
                new_def.active as i64,
                new_def.filterable as i64,
                new_def.stat_enabled as i64,
                new_def.analysis_enabled as i64,
                new_def.display_order,
                new_def.form_group,
                serde_json::to_string(&new_def.validation_rules).map_err(|e| SchemaError::storage(e.to_string()))?,
                new_def.schema_version as i64,
                now_iso(),
            ],
        )?;
        self.get_field(field_id)
    }

    /// غیرفعال‌سازی فیلد — داده‌ها حفظ می‌شوند (حذف سخت ممنوع است).
    pub fn deactivate_field(&self, field_id: &str) -> Result<(), SchemaError> {
        self.get_field(field_id)?;
        self.db.lock().execute(
            "UPDATE custom_fields SET active = 0, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![field_id, now_iso()],
        )?;
        tracing::info!(field = field_id, "custom field deactivated");
        Ok(())
    }

    /// آیا فیلد داده ذخیره‌شده دارد.
    pub fn field_has_data(&self, field_id: &str) -> Result<bool, SchemaError> {
        let conn = self.db.lock();
        let n: i64 = conn.query_row(
            "SELECT count(*) FROM field_values WHERE field_id = ?1",
            [field_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    // ================= مقادیر فیلدها =================

    /// ذخیره مقدار یک فیلد برای یک معامله (اعتبارسنجی کامل قبل از نوشتن).
    pub fn set_value(
        &self,
        trade_id: &str,
        field_id: &str,
        raw: &serde_json::Value,
    ) -> Result<(), SchemaError> {
        let field = self.get_field(field_id)?;
        let options = self.active_options(field_id)?;
        let typed = validate_value(&field, &options, raw)?;
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO field_values (trade_id, field_id, text_value, integer_value, decimal_value,
             boolean_value, datetime_value, json_value, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(trade_id, field_id) DO UPDATE SET
               text_value=?3, integer_value=?4, decimal_value=?5, boolean_value=?6,
               datetime_value=?7, json_value=?8, updated_at=?9",
            rusqlite::params![
                trade_id,
                field_id,
                match &typed { TypedValue::Text(s) => Some(s), _ => None },
                match &typed { TypedValue::Integer(n) => Some(*n), _ => None },
                match &typed { TypedValue::Decimal(n) => Some(*n), _ => None },
                match &typed { TypedValue::Boolean(b) => Some(*b as i64), _ => None },
                match &typed { TypedValue::Datetime(s) => Some(s), _ => None },
                match &typed { TypedValue::Json(v) => Some(v.to_string()), _ => None },
                now_iso(),
            ],
        )?;
        Ok(())
    }

    /// پاک کردن مقدار فیلد برای یک معامله.
    pub fn clear_value(&self, trade_id: &str, field_id: &str) -> Result<(), SchemaError> {
        self.db.lock().execute(
            "DELETE FROM field_values WHERE trade_id = ?1 AND field_id = ?2",
            rusqlite::params![trade_id, field_id],
        )?;
        Ok(())
    }

    /// مقادیر همه فیلدهای یک معامله — خروجی JSON: {technical_key: value}.
    pub fn get_values(&self, trade_id: &str) -> Result<serde_json::Value, SchemaError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT f.technical_key, f.storage_type, v.text_value, v.integer_value, v.decimal_value,
             v.boolean_value, v.datetime_value, v.json_value
             FROM field_values v JOIN custom_fields f ON f.id = v.field_id
             WHERE v.trade_id = ?1 ORDER BY f.technical_key",
        )?;
        let rows = stmt.query_map([trade_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut map = serde_json::Map::new();
        for row in rows {
            let (key, st_text, text_v, int_v, dec_v, bool_v, dt_v, json_v) = row?;
            let storage = StorageType::parse(&st_text)
                .ok_or_else(|| SchemaError::storage(format!("corrupt storage_type: {st_text}")))?;
            let value = match storage {
                StorageType::Text | StorageType::LongText | StorageType::Enum => {
                    text_v.map(serde_json::Value::String)
                }
                StorageType::MultiEnum | StorageType::TagSet => json_v.and_then(|s| serde_json::from_str(&s).ok()),
                StorageType::Integer | StorageType::Rating => int_v.map(|n| serde_json::json!(n)),
                StorageType::Decimal => dec_v.map(|n| serde_json::json!(n)),
                StorageType::Boolean => bool_v.map(|b| serde_json::json!(b != 0)),
                StorageType::Datetime => dt_v.map(serde_json::Value::String),
            };
            if let Some(v) = value {
                map.insert(key, v);
            }
        }
        Ok(serde_json::Value::Object(map))
    }

    /// گزینه‌های فعال یک فیلد (مرتب).
    pub fn active_options(&self, field_id: &str) -> Result<Vec<FieldOption>, SchemaError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, field_id, value, label, sort_order, active
             FROM field_options WHERE field_id = ?1 AND active = 1
             ORDER BY sort_order, value",
        )?;
        let rows = stmt.query_map([field_id], |r| {
            Ok(FieldOption {
                id: r.get(0)?,
                field_id: r.get(1)?,
                value: r.get(2)?,
                label: r.get(3)?,
                sort_order: r.get(4)?,
                active: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn map_field_row(r: &rusqlite::Row<'_>) -> Result<FieldDefinition, rusqlite::Error> {
    let storage_text: String = r.get(4)?;
    let semantic_text: String = r.get(5)?;
    let rules_text: String = r.get(15)?;
    let storage_type = StorageType::parse(&storage_text).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            format!("unknown storage_type: {storage_text}").into(),
        )
    })?;
    let semantic_type = SemanticType::parse(&semantic_text).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            format!("unknown semantic_type: {semantic_text}").into(),
        )
    })?;
    Ok(FieldDefinition {
        id: r.get(0)?,
        technical_key: r.get(1)?,
        display_label: r.get(2)?,
        description: r.get(3)?,
        storage_type,
        semantic_type,
        unit: r.get(6)?,
        default_value: r.get::<_, Option<String>>(7)?.and_then(|s| serde_json::from_str(&s).ok()),
        required: r.get::<_, i64>(8)? != 0,
        active: r.get::<_, i64>(9)? != 0,
        filterable: r.get::<_, i64>(10)? != 0,
        stat_enabled: r.get::<_, i64>(11)? != 0,
        analysis_enabled: r.get::<_, i64>(12)? != 0,
        display_order: r.get(13)?,
        form_group: r.get(14)?,
        validation_rules: serde_json::from_str(&rules_text)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(15, rusqlite::types::Type::Text, Box::new(e)))?,
        schema_version: r.get::<_, i64>(16)? as u32,
    })
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// خروجی فرم برای یک فیلد — نوع ویجت از نوع معنایی استنتاج می‌شود.
fn field_form_json(f: &FieldDefinition) -> serde_json::Value {
    let widget = match f.semantic_type {
        crate::model::SemanticType::LongText => "textarea",
        crate::model::SemanticType::Boolean => "checkbox",
        crate::model::SemanticType::SingleSelect | crate::model::SemanticType::Rating => "select",
        crate::model::SemanticType::MultiSelect | crate::model::SemanticType::Tag => "multi_select",
        crate::model::SemanticType::Datetime => "datetime_picker",
        _ => "input",
    };
    serde_json::json!({
        "id": f.id,
        "key": f.technical_key,
        "label": f.display_label,
        "widget": widget,
        "required": f.required,
        "unit": f.unit,
        "default": f.default_value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SemanticType, ValidationRules};
    use aria_storage_engine::migrations::run_migrations;

    fn setup_db() -> Database {
        let db = Database::open_memory(None).unwrap();
        {
            let mut conn = db.lock();
            run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    fn svc(db: &Database) -> SchemaService<'_> {
        SchemaService::new(db)
    }

    /// ساخت یک معامله معتبر (زنجیره FK: پروفایل → حساب → نماد → معامله).
    fn make_trade(db: &Database) -> String {
        let conn = db.lock();
        let now = now_iso();
        let pid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO profiles (id, name, created_at, updated_at) VALUES (?1, 'p' || ?1, ?2, ?2)",
            rusqlite::params![pid, now],
        )
        .unwrap();
        let aid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO trading_accounts (id, profile_id, name, currency, created_at, updated_at)
             VALUES (?1, ?2, 'حساب تست', 'USD', ?3, ?3)",
            rusqlite::params![aid, pid, now],
        )
        .unwrap();
        let sid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO symbols (id, name, created_at) VALUES (?1, 's' || ?1, ?2)",
            rusqlite::params![sid, now],
        )
        .unwrap();
        let tid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO journal_trades (id, account_id, symbol_id, direction, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'buy', ?4, ?4)",
            rusqlite::params![tid, aid, sid, now],
        )
        .unwrap();
        tid
    }

    #[test]
    fn define_list_and_get_field() {
        let db = setup_db();
        let s = svc(&db);
        let f1 = s
            .define_field(FieldDefinition::new("setup_quality", "کیفیت ستاپ", StorageType::Enum, SemanticType::SingleSelect))
            .unwrap();
        let f2 = s.define_field(FieldDefinition::new("risk_percent", "درصد ریسک", StorageType::Decimal, SemanticType::Percent)).unwrap();

        let all = s.list_fields(true).unwrap();
        assert_eq!(all.len(), 2);

        let got = s.get_field(&f1.id).unwrap();
        assert_eq!(got.technical_key, "setup_quality");

        let by_key = s.find_by_key("risk_percent").unwrap().unwrap();
        assert_eq!(by_key.id, f2.id);
        assert!(s.find_by_key("nope").unwrap().is_none());
    }

    #[test]
    fn duplicate_technical_key_rejected() {
        let db = setup_db();
        let s = svc(&db);
        s.define_field(FieldDefinition::new("dup_key", "الف", StorageType::Text, SemanticType::ShortText)).unwrap();
        let err = s
            .define_field(FieldDefinition::new("dup_key", "ب", StorageType::Integer, SemanticType::Number))
            .unwrap_err();
        assert_eq!(err.code(), 1302);
    }

    #[test]
    fn invalid_definition_rejected() {
        let db = setup_db();
        let s = svc(&db);
        let err = s.define_field(FieldDefinition::new("Bad-Key", "x", StorageType::Text, SemanticType::ShortText)).unwrap_err();
        assert_eq!(err.code(), 1303);
        let err2 = s.define_field(FieldDefinition::new("ok_key", " ", StorageType::Text, SemanticType::ShortText)).unwrap_err();
        assert_eq!(err2.code(), 1303);
    }

    #[test]
    fn options_lifecycle() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("mood", "حال و هوا", StorageType::Enum, SemanticType::SingleSelect)).unwrap();
        let o1 = s.add_option(&f.id, "calm", "آرام", 1).unwrap();
        s.add_option(&f.id, "fomo", "فومو", 2).unwrap();
        assert!(matches!(
            s.add_option(&f.id, "calm", "تکراری", 3),
            Err(SchemaError::InvalidDefinition { .. })
        ));
        // گزینه برای فیلد غیرselect ممنوع
        let f2 = s.define_field(FieldDefinition::new("num", "عدد", StorageType::Integer, SemanticType::Number)).unwrap();
        assert!(s.add_option(&f2.id, "x", "x", 1).is_err());

        let opts = s.active_options(&f.id).unwrap();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].value, "calm");
        let _ = o1;
    }

    #[test]
    fn set_and_get_values_all_types() {
        let db = setup_db();
        let s = svc(&db);
        let f_text = s.define_field(FieldDefinition::new("note2", "یادداشت", StorageType::Text, SemanticType::ShortText)).unwrap();
        let f_int = s.define_field(FieldDefinition::new("screens", "اسکرین", StorageType::Integer, SemanticType::Number)).unwrap();
        let f_dec = s.define_field(FieldDefinition::new("rr", "نسبت ریسک", StorageType::Decimal, SemanticType::Number)).unwrap();
        let f_bool = s.define_field(FieldDefinition::new("planned", "برنامه‌ریزی‌شده", StorageType::Boolean, SemanticType::Boolean)).unwrap();
        let f_dt = s.define_field(FieldDefinition::new("review_at", "زمان بازبینی", StorageType::Datetime, SemanticType::Datetime)).unwrap();
        let f_tags = s.define_field(FieldDefinition::new("tags2", "برچسب‌ها", StorageType::TagSet, SemanticType::Tag)).unwrap();

        let trade = make_trade(&db);
        s.add_option(&f_tags.id, "breakout", "بریک‌اوت", 1).unwrap();
        s.add_option(&f_tags.id, "high_volume", "حجم بالا", 2).unwrap();
        s.set_value(&trade, &f_tags.id, &serde_json::json!(["breakout", "high_volume"])).unwrap();
        s.set_value(&trade, &f_text.id, &serde_json::json!("ورود بر اساس ستاپ")).unwrap();
        s.set_value(&trade, &f_int.id, &serde_json::json!(3)).unwrap();
        s.set_value(&trade, &f_dec.id, &serde_json::json!(2.5)).unwrap();
        s.set_value(&trade, &f_bool.id, &serde_json::json!(true)).unwrap();
        s.set_value(&trade, &f_dt.id, &serde_json::json!("2026-09-28T08:30:00Z")).unwrap();

        let values = s.get_values(&trade).unwrap();
        assert_eq!(values["note2"], "ورود بر اساس ستاپ");
        assert_eq!(values["screens"], 3);
        assert_eq!(values["rr"], 2.5);
        assert_eq!(values["planned"], true);
        assert_eq!(values["review_at"], "2026-09-28T08:30:00Z");
        assert_eq!(values["tags2"], serde_json::json!(["breakout", "high_volume"]));
    }

    #[test]
    fn set_value_validates_and_rejects() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("rating1", "امتیاز", StorageType::Rating, SemanticType::Rating)).unwrap();
        let t1 = make_trade(&db);
        assert!(matches!(
            s.set_value(&t1, &f.id, &serde_json::json!(9)),
            Err(SchemaError::InvalidValue { .. })
        ));
        s.set_value(&t1, &f.id, &serde_json::json!(4)).unwrap();
        // مقدار قبلی جایگزین می‌شود
        s.set_value(&t1, &f.id, &serde_json::json!(2)).unwrap();
        let v = s.get_values(&t1).unwrap();
        assert_eq!(v["rating1"], 2);
    }

    #[test]
    fn enum_option_required_for_value() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("mistake1", "اشتباه", StorageType::Enum, SemanticType::SingleSelect)).unwrap();
        let t1 = make_trade(&db);
        s.add_option(&f.id, "early_entry", "ورود زودهنگام", 1).unwrap();
        assert!(matches!(
            s.set_value(&t1, &f.id, &serde_json::json!("late_entry")),
            Err(SchemaError::UnknownOption { .. })
        ));
        s.set_value(&t1, &f.id, &serde_json::json!("early_entry")).unwrap();
    }

    #[test]
    fn update_field_rules_and_type_change_policy() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("percent1", "درصد", StorageType::Decimal, SemanticType::Percent)).unwrap();

        // به‌روزرسانی آزاد برچسب/قواعد
        let mut updated = s.get_field(&f.id).unwrap();
        updated.display_label = "درصد ریسک".to_string();
        updated.validation_rules.max = Some(100.0);
        s.update_field(&f.id, updated).unwrap();
        let after = s.get_field(&f.id).unwrap();
        assert_eq!(after.display_label, "درصد ریسک");
        assert_eq!(after.validation_rules.max, Some(100.0));

        // تغییر نوع قبل از داده مجاز است
        let mut to_int = after.clone();
        to_int.storage_type = StorageType::Integer;
        s.update_field(&f.id, to_int).unwrap();

        // با داده → تغییر نوع ممنوع
        let t9 = make_trade(&db);
        s.set_value(&t9, &f.id, &serde_json::json!(5)).unwrap();
        let mut back_to_dec = s.get_field(&f.id).unwrap();
        back_to_dec.storage_type = StorageType::Decimal;
        let err = s.update_field(&f.id, back_to_dec).unwrap_err();
        assert_eq!(err.code(), 1305);
    }

    #[test]
    fn deactivate_keeps_data_and_hides_field() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("conf1", "اطمینان", StorageType::Integer, SemanticType::Number)).unwrap();
        let t1 = make_trade(&db);
        s.set_value(&t1, &f.id, &serde_json::json!(7)).unwrap();
        assert!(s.field_has_data(&f.id).unwrap());

        s.deactivate_field(&f.id).unwrap();
        assert!(!s.list_fields(false).unwrap().iter().any(|x| x.id == f.id));
        assert!(s.list_fields(true).unwrap().iter().any(|x| x.id == f.id));
        // داده حفظ شده است
        let v = s.get_values(&t1).unwrap();
        assert_eq!(v["conf1"], 7);
    }

    #[test]
    fn clear_value() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("x1", "ایکس", StorageType::Integer, SemanticType::Number)).unwrap();
        let t1 = make_trade(&db);
        s.set_value(&t1, &f.id, &serde_json::json!(1)).unwrap();
        s.clear_value(&t1, &f.id).unwrap();
        assert!(s.get_values(&t1).unwrap().as_object().unwrap().is_empty());
    }

    #[test]
    fn filterable_fields_and_form_metadata() {
        let db = setup_db();
        let s = svc(&db);
        let mut f1 = FieldDefinition::new("market1", "بازار", StorageType::Enum, SemanticType::SingleSelect);
        f1.filterable = true;
        f1.form_group = Some("بازار".to_string());
        f1.display_order = 1;
        let mut f2 = FieldDefinition::new("emotion1", "احساس", StorageType::Enum, SemanticType::SingleSelect);
        f2.filterable = true;
        f2.form_group = Some("روانشناسی".to_string());
        f2.display_order = 2;
        let mut f3 = FieldDefinition::new("hidden1", "پنهان", StorageType::Text, SemanticType::ShortText);
        f3.filterable = false;
        s.define_field(f1).unwrap();
        s.define_field(f2).unwrap();
        s.define_field(f3).unwrap();

        let flt = s.filterable_fields().unwrap();
        assert_eq!(flt.len(), 2);

        let meta = s.form_metadata().unwrap();
        let groups = meta["groups"].as_array().unwrap();
        // دو گروه تعریف‌شده + گروه پیش‌فرض «عمومی» برای فیلد بدون گروه
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().any(|g| g["group"] == "بازار" && g["fields"].as_array().unwrap().len() == 1));
        assert!(groups.iter().any(|g| g["group"] == "عمومی"));
        // ویجت از نوع معنایی
        let market_group = groups.iter().find(|g| g["group"] == "بازار").unwrap();
        assert_eq!(market_group["fields"][0]["widget"], "select");
    }

    #[test]
    fn required_flag_roundtrip() {
        let db = setup_db();
        let s = svc(&db);
        let mut f = FieldDefinition::new("must1", "اجباری", StorageType::Text, SemanticType::ShortText);
        f.required = true;
        let saved = s.define_field(f).unwrap();
        let got = s.get_field(&saved.id).unwrap();
        assert!(got.required);
    }

    #[test]
    fn validation_rules_enforced_through_service() {
        let db = setup_db();
        let s = svc(&db);
        let mut f = FieldDefinition::new("code1", "کد", StorageType::Text, SemanticType::ShortText);
        f.validation_rules = ValidationRules { min_length: Some(3), max_length: None, min: None, max: None };
        let saved = s.define_field(f).unwrap();
        let t1 = make_trade(&db);
        assert!(s.set_value(&t1, &saved.id, &serde_json::json!("ab")).is_err());
        assert!(s.set_value(&t1, &saved.id, &serde_json::json!("abc")).is_ok());
    }

    #[test]
    fn values_are_per_trade_isolated() {
        let db = setup_db();
        let s = svc(&db);
        let f = s.define_field(FieldDefinition::new("iso1", "جداسازی", StorageType::Integer, SemanticType::Number)).unwrap();
        let t1 = make_trade(&db);
        let t2 = make_trade(&db);
        s.set_value(&t1, &f.id, &serde_json::json!(10)).unwrap();
        s.set_value(&t2, &f.id, &serde_json::json!(20)).unwrap();
        assert_eq!(s.get_values(&t1).unwrap()["iso1"], 10);
        assert_eq!(s.get_values(&t2).unwrap()["iso1"], 20);
    }
}
