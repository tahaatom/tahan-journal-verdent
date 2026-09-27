@echo off
rem قالب‌بندی همه کدها (rustfmt + prettier)
cd /d "%~dp0.."
cargo fmt --all
cd frontend
npm run format
