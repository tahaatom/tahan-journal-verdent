@echo off
rem ============================================================
rem  اجراکننده ژورنال طهان — با دابل‌کلیک برنامه باز می‌شود
rem  اگر نسخه ساخته‌شده موجود باشد مستقیماً اجرا می‌شود؛
rem  در غیر این صورت حالت توسعه ساخته و اجرا می‌گردد.
rem ============================================================
setlocal
chcp 65001 >nul
set "ROOT=%~dp0"
set "EXE="

rem جست‌وجوی نسخه ساخته‌شده در مسیرهای محتمل (پوشه هدف سفارشی یا پیش‌فرض)
for %%P in (
    "D:\tahan-target\release\tahan-desktop.exe"
    "D:\tahan-target\debug\tahan-desktop.exe"
    "%ROOT%target\release\tahan-desktop.exe"
    "%ROOT%target\debug\tahan-desktop.exe"
    "%ROOT%apps\tahan-desktop\target\release\tahan-desktop.exe"
    "%ROOT%apps\tahan-desktop\target\debug\tahan-desktop.exe"
) do (
    if not defined EXE if exist %%P set "EXE=%%~P"
)

if defined EXE (
    start "" "%EXE%"
    exit /b 0
)

echo [ژورنال طهان] نسخه ساخته‌شده یافت نشد؛ ساخت و اجرای حالت توسعه...
echo این ممکن است چند دقیقه طول بکشد؛ پنجره را نبندید.
where cargo >nul 2>nul
if errorlevel 1 (
    echo خطا: Rust نصب نیست یا در PATH نیست.
    pause
    exit /b 1
)
cd /d "%ROOT%apps\tahan-desktop"
set "CARGO_TARGET_DIR=D:\tahan-target"
cargo run
if errorlevel 1 (
    echo خطا در اجرای برنامه — جزئیات در پیام‌های بالا.
    pause
    exit /b 1
)
