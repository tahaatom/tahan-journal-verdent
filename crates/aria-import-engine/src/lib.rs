//! موتور ایمپورت متاتریدر — پلاگین رسمی ژورنال طهان (فاز ۱.۱۵).
//!
//! جریان ایمپورت: رمزگشایی → تجزیه (CSV/HTML) → نگاشت کانونی →
//! تشخیص تکرار → گروه‌بندی پوزیشن → ساخت معامله/پا/اجرا در یک تراکنش.

pub mod decode;
pub mod dedup;
pub mod error;
pub mod map;
pub mod model;
pub mod parse;
pub mod service;

pub use decode::{decode, Decoded};
pub use dedup::{is_file_duplicate, is_row_duplicate};
pub use error::ImportError;
pub use map::{map_file, parse_f64, parse_mt_time, MapOutcome};
pub use model::{DealRole, DealSide, ImportReport, MappedDeal, SourceFormat};
pub use parse::{detect_format, row_hash};
pub use service::{ImportService, MAX_IMPORT_BYTES};
