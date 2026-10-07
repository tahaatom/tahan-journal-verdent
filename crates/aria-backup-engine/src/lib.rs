//! موتور بکاپ و بازیابی رمزنگاری‌شده ژورنال طهان (فاز ۱.۱۶).
//!
//! قالب بسته `.tahanbak`:
//!
//! ```text
//! magic "TAHANBAK" || u32 نسخه ظرف || u32 طول مانیفست || مانیفست JSON
//!                  || u64 طول بار  || بار = AES-256-GCM(zstd(بسته درونی))
//! ```
//!
//! کلید با argon2id از گذرواژه مشتق می‌شود؛ مانیفست به‌عنوان AAD احراز
//! می‌شود و بازیابی اتمیک با نسخه امنیتی و بازگشت خودکار انجام می‌گیرد.

pub mod bundle;
pub mod container;
pub mod error;
pub mod manifest;
pub mod service;

pub use bundle::BundleEntry;
pub use error::BackupError;
pub use manifest::{BackupManifest, EntryMeta, KdfParams, PACKAGE_FORMAT_VERSION};
pub use service::{
    check_manifest_entries, create_backup, inspect_package, prune_safety_copies, restore_backup,
    snapshot_database, verify_package, BackupReport, RestoreReport, Workspace,
};
