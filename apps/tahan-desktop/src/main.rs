//! پوسته دسکتاپ ژورنال طهان — اتصال فرانت‌اند به کرنل آریا
//!
//! در فاز ۰ فقط یک دستور ping برای صحت IPC اضافه می‌شود.
//! در فازهای بعد، دستورات امن کرنل اینجا به فرانت‌اند عرضه می‌شوند.

#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("خطا در اجرای ژورنال طهان");
}
