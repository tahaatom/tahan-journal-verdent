@echo off
rem اجرای همه تست‌ها (Rust + فرانت‌اند)
cd /d "%~dp0.."
cargo test --workspace || exit /b 1
cd frontend
npm run test -- --run || exit /b 1
