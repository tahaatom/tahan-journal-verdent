@echo off
rem اجرای همه لینت‌ها (clippy + eslint)
cd /d "%~dp0.."
cargo clippy --workspace --all-targets -- -D warnings || exit /b 1
cd frontend
npm run lint || exit /b 1
