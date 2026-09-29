//! نوع سبک فیلد سفارشی برای منطق خالص — هم‌تراز `CustomFieldDef` در kernel.ts.
//!
//! جدا از kernel.ts تا منطق اعتبارسنجی/ثبت به پل IPC وابسته نباشد.

export interface CustomFieldDefLike {
  id: string;
  technical_key: string;
  display_label: string;
  storage_type: string;
  semantic_type: string;
  unit: string | null;
  required: boolean;
  display_order: number;
  form_group: string | null;
  validation_rules: {
    min?: number | null;
    max?: number | null;
    min_length?: number | null;
    max_length?: number | null;
  };
}

/** گزینه فعال فیلد انتخابی — هم‌تراز `FieldOptionDto`. */
export interface FieldOptionLike {
  value: string;
  label: string;
}
