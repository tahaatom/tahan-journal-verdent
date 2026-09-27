@echo off
rem اجرای بک‌اند (کرنل آریا) در حالت توسعه — فعلاً build/watch کل ورک‌اسپیس
cargo watch -- sh -c "cargo build --workspace" 2>nul || cargo build --workspace
