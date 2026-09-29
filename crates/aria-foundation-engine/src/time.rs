//! سرویس زمان موتور پایه.
//!
//! قوانین (سند نسخه ۵):
//! - زمان ذخیره‌سازی همیشه UTC و قالب ISO 8601 است.
//! - نمایش با تقویم جلالی انجام می‌شود.
//! - ابزار سشن معاملاتی به‌عنوان قلاب پایه در نسخه ۱ ارائه می‌شود.

use crate::error::FoundationError;
use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};

/// تبدیل میلادی به جلالی (الگوریتم استاندارد جلالی، بازه معتبر ۱۱۷۸ تا ۱۶۳۳ شمسی).
pub fn gregorian_to_jalali(gy: i32, gm: u32, gd: u32) -> (i32, u32, u32) {
    const G_DAYS: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    const J_DAYS: [i32; 12] = [31, 31, 31, 31, 31, 31, 30, 30, 30, 30, 30, 29];

    let gy2 = gy - 1600;
    let gm2 = (gm - 1) as i32;
    let gd2 = (gd - 1) as i32;

    let mut g_day_no = 365 * gy2 + (gy2 + 3) / 4 - (gy2 + 99) / 100 + (gy2 + 399) / 400;
    for i in 0..gm2 {
        g_day_no += G_DAYS[i as usize];
    }
    if gm2 > 1 && ((gy % 4 == 0 && gy % 100 != 0) || gy % 400 == 0) {
        g_day_no += 1;
    }
    g_day_no += gd2;

    let mut j_day_no = g_day_no - 79;
    let j_np = j_day_no / 12053;
    j_day_no %= 12053;
    let mut jy = 979 + 33 * j_np + 4 * (j_day_no / 1461);
    j_day_no %= 1461;
    if j_day_no >= 366 {
        jy += (j_day_no - 1) / 365;
        j_day_no = (j_day_no - 1) % 365;
    }
    let mut jm = 0;
    let mut jd = 0;
    for (i, d) in J_DAYS.iter().enumerate() {
        if j_day_no < *d {
            jm = i as u32 + 1;
            jd = j_day_no as u32 + 1;
            break;
        }
        j_day_no -= *d;
    }
    if jm == 0 {
        // روز ۳۰ اسفند در سال کبیسه: پس از کسر کامل ۱۲ ماه (۳۶۵ روز جدول) باقی مانده
        jm = 12;
        jd = j_day_no as u32 + 30;
    }
    (jy, jm, jd)
}

/// هسته تبدیل جلالی به میلادی بدون اعتبارسنجی (فقط برای مقادیر مجاز داخلی).
fn jalali_to_gregorian_raw(jy: i32, jm: u32, jd: u32) -> (i32, u32, u32) {
    const G_DAYS: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    const J_DAYS: [i32; 12] = [31, 31, 31, 31, 31, 31, 30, 30, 30, 30, 30, 29];

    let jy2 = jy - 979;
    let jm2 = (jm - 1) as i32;
    let jd2 = (jd - 1) as i32;

    let mut j_day_no = 365 * jy2 + (jy2 / 33) * 8 + (jy2 % 33 + 3) / 4;
    for i in 0..jm2 {
        j_day_no += J_DAYS[i as usize];
    }
    j_day_no += jd2;

    let mut g_day_no = j_day_no + 79;
    let mut gy = 1600 + 400 * (g_day_no / 146097);
    g_day_no %= 146097;
    let mut leap = true;
    if g_day_no >= 36525 {
        g_day_no -= 1;
        gy += 100 * (g_day_no / 36524);
        g_day_no %= 36524;
        if g_day_no >= 365 {
            g_day_no += 1;
        } else {
            leap = false;
        }
    }
    gy += 4 * (g_day_no / 1461);
    g_day_no %= 1461;
    if g_day_no >= 366 {
        leap = false;
        g_day_no -= 1;
        gy += g_day_no / 365;
        g_day_no %= 365;
    }
    let mut gm = 0;
    let mut gd = 0;
    for (i, d) in G_DAYS.iter().enumerate() {
        let mut i_days = *d;
        if i == 1 && leap {
            i_days += 1;
        }
        if g_day_no < i_days {
            gm = i as u32 + 1;
            gd = g_day_no as u32 + 1;
            break;
        }
        g_day_no -= i_days;
    }
    (gy, gm, gd)
}

/// تبدیل جلالی به میلادی. اگر تاریخ نامعتبر باشد خطا برمی‌گرداند.
pub fn jalali_to_gregorian(jy: i32, jm: u32, jd: u32) -> Result<(i32, u32, u32), FoundationError> {
    const J_DAYS: [i32; 12] = [31, 31, 31, 31, 31, 31, 30, 30, 30, 30, 30, 29];

    if !(1..=12).contains(&jm) {
        return Err(FoundationError::invalid_datetime(format!(
            "jalali month out of range: {jm}"
        )));
    }
    let max_day: u32 = if jm == 12 {
        // طول واقعی اسفند از فاصله دو نوروز محاسبه می‌شود (بدون بازگشت به این تابع)
        let r1 = jalali_to_gregorian_raw(jy, 12, 1);
        let r2 = jalali_to_gregorian_raw(jy + 1, 1, 1);
        let d1 = NaiveDate::from_ymd_opt(r1.0, r1.1, r1.2).expect("raw conversion of valid month start");
        let d2 = NaiveDate::from_ymd_opt(r2.0, r2.1, r2.2).expect("raw conversion of valid month start");
        (d2 - d1).num_days() as u32
    } else {
        J_DAYS[(jm - 1) as usize] as u32
    };
    if jd < 1 || jd > max_day {
        return Err(FoundationError::invalid_datetime(format!(
            "jalali day out of range: {jd}"
        )));
    }
    if !(1178..=1633).contains(&jy) {
        return Err(FoundationError::invalid_datetime(format!(
            "jalali year out of supported range: {jy}"
        )));
    }

    Ok(jalali_to_gregorian_raw(jy, jm, jd))
}

/// تعداد روزهای ماه جلالی — خطا به‌جای panic برای ماه/سال نامعتبر.
pub fn jalali_month_days(jy: i32, jm: u32) -> Result<u32, FoundationError> {
    const J_DAYS: [u32; 12] = [31, 31, 31, 31, 31, 31, 30, 30, 30, 30, 30, 29];
    if !(1..=12).contains(&jm) {
        return Err(FoundationError::invalid_datetime(format!(
            "jalali month out of range: {jm}"
        )));
    }
    // سال خارج از بازه پشتیبانی‌شده همیشه خطاست — حتی برای ماه‌های با طول ثابت
    if !(1178..=1633).contains(&jy) {
        return Err(FoundationError::invalid_datetime(format!(
            "jalali year out of supported range: {jy}"
        )));
    }
    if jm != 12 {
        return Ok(J_DAYS[(jm - 1) as usize]);
    }
    // طول اسفند = فاصله روز اول اسفند تا نوروز سال بعد
    // (سال ۱۶۳۳ به نوروز ۱۶۳۴ نیاز دارد که بیرون بازه است → خطا، نه panic)
    let d1 = jalali_date_to_naive(jy, 12, 1)?;
    let d2 = jalali_date_to_naive(jy + 1, 1, 1)?;
    Ok((d2 - d1).num_days() as u32)
}

/// تاریخ جلالی قالب‌بندی‌شده (مثل «۱۴۰۵/۰۷/۰۵»).
pub fn format_jalali(date: NaiveDate) -> String {
    let (jy, jm, jd) = gregorian_to_jalali(date.year(), date.month(), date.day());
    format!("{jy:04}/{jm:02}/{jd:02}")
}

/// نمایش کامل تاریخ-ساعت UTC با تقویم جلالی (برای رابط کاربری).
pub fn display_jalali_datetime_utc(dt: DateTime<Utc>) -> String {
    let local = dt.with_timezone(&chrono::Local);
    format!("{} {:02}:{:02}", format_jalali(local.date_naive()), local.hour(), local.minute())
}

/// تبدیل تاریخ جلالی به NaiveDate میلادی.
pub fn jalali_date_to_naive(jy: i32, jm: u32, jd: u32) -> Result<NaiveDate, FoundationError> {
    let (gy, gm, gd) = jalali_to_gregorian(jy, jm, jd)?;
    NaiveDate::from_ymd_opt(gy, gm, gd)
        .ok_or_else(|| FoundationError::invalid_datetime(format!("{jy}/{jm}/{jd}")))
}

/// زمان فعلی به‌صورت ISO 8601 UTC (قالب ذخیره‌سازی استاندارد).
pub fn now_utc_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// پارس رشته ISO 8601 به DateTime UTC.
pub fn parse_utc_iso(s: &str) -> Result<DateTime<Utc>, FoundationError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| FoundationError::invalid_datetime(e.to_string()))
}

/// سشن معاملاتی پایه (نسخه ۱) بر اساس ساعت UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingSession {
    /// سیدنی/آسیا
    Asia,
    /// لندن/اروپا
    Europe,
    /// لندن-نیویورک هم‌پوشان
    Overlap,
    /// نیویورک/آمریکا
    America,
    /// خارج از سشن‌های اصلی
    OffHours,
}

/// تشخیص سشن معاملاتی از ساعت UTC (قلاب پایه نسخه ۱؛ قابل ارتقاء در فازهای بعد).
pub fn trading_session_utc(dt: DateTime<Utc>) -> TradingSession {
    let h = dt.hour();
    match h {
        0..=6 => TradingSession::Asia,
        7..=11 => TradingSession::Europe,
        12..=15 => TradingSession::Overlap,
        16..=20 => TradingSession::America,
        _ => TradingSession::OffHours,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn gregorian_to_jalali_known_dates() {
        // نوزدهم مهر ۱۴۰۵
        assert_eq!(gregorian_to_jalali(2026, 10, 11), (1405, 7, 19));
        // نوروز ۱۴۰۳ = ۲۰ مارس ۲۰۲۴
        assert_eq!(gregorian_to_jalali(2024, 3, 20), (1403, 1, 1));
        // نوروز ۱۴۰۴ = ۲۱ مارس ۲۰۲۵
        assert_eq!(gregorian_to_jalali(2025, 3, 21), (1404, 1, 1));
        // نوروز ۱۴۰۵ = ۲۱ مارس ۲۰۲۶
        assert_eq!(gregorian_to_jalali(2026, 3, 21), (1405, 1, 1));
        // پایان اسفند سال غیرکبیسه ۱۴۰۳ = ۱۹ مارس ۲۰۲۵
        assert_eq!(gregorian_to_jalali(2025, 3, 20), (1403, 12, 30));
    }

    #[test]
    fn jalali_gregorian_roundtrip_range() {
        // رفت‌وبرگشت روی بازه وسیع: از ۱۴۰۰/۱/۱ تا ۱۴۱۰/۱۲/۲۹
        let (sy, sm) = (1400, 1);
        let (ey, em) = (1410, 12);
        let mut count = 0u32;
        let mut y = sy;
        let mut m = sm;
        loop {
            let days = if m == 12 {
                jalali_month_days(y, 12).expect("roundtrip year in range")
            } else {
                [31, 31, 31, 31, 31, 31, 30, 30, 30, 30, 30][m as usize - 1]
            };
            for d in 1..=days {
                let g = jalali_to_gregorian(y, m, d).expect("valid date");
                assert_eq!(gregorian_to_jalali(g.0, g.1, g.2), (y, m, d), "roundtrip failed for {y}/{m}/{d}");
                count += 1;
            }
            if (y, m) == (ey, em) {
                break;
            }
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
        assert!(count > 3500);
    }

    #[test]
    fn invalid_jalali_dates_rejected() {
        assert!(jalali_to_gregorian(1405, 13, 1).is_err());
        assert!(jalali_to_gregorian(1405, 1, 0).is_err());
        assert!(jalali_to_gregorian(1403, 12, 31).is_err()); // اسفند حداکثر ۳۰ روز دارد
        assert!(jalali_to_gregorian(100, 1, 1).is_err()); // خارج بازه
    }

    #[test]
    fn month_days_out_of_range_errors_not_panics() {
        // ماه نامعتبر
        assert!(jalali_month_days(1405, 13).is_err());
        assert!(jalali_month_days(1405, 0).is_err());
        // سال مرزی بالای بازه — اسفند ۱۶۳۳ به نوروز ۱۶۳۴ نیاز دارد که بیرون بازه است
        assert!(jalali_month_days(1633, 12).is_err());
        // سال پایین بازه
        assert!(jalali_month_days(1177, 1).is_err());
        // مقادیر معتبر
        assert_eq!(jalali_month_days(1405, 7).unwrap(), 30);
        assert_eq!(jalali_month_days(1403, 12).unwrap(), 30); // کبیسه
        assert_eq!(jalali_month_days(1404, 12).unwrap(), 29); // غیرکبیسه
    }

    #[test]
    fn iso_roundtrip() {
        // دقت ذخیره‌سازی میکروثانیه است؛ پس از رفت‌وبرگشت، میکروثانیه‌ها باید برابر بمانند
        let now = Utc::now();
        let iso = now.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        let back = parse_utc_iso(&iso).unwrap();
        assert_eq!(back.timestamp_micros(), now.timestamp_micros());
        assert!(parse_utc_iso("not-a-date").is_err());
    }

    #[test]
    fn jalali_display_format() {
        let d = jalali_date_to_naive(1405, 7, 5).unwrap();
        assert_eq!(format_jalali(d), "1405/07/05");
    }

    #[test]
    fn trading_sessions() {
        let mk = |h: u32| Utc.with_ymd_and_hms(2026, 1, 1, h, 0, 0).unwrap();
        assert_eq!(trading_session_utc(mk(3)), TradingSession::Asia);
        assert_eq!(trading_session_utc(mk(9)), TradingSession::Europe);
        assert_eq!(trading_session_utc(mk(13)), TradingSession::Overlap);
        assert_eq!(trading_session_utc(mk(18)), TradingSession::America);
        assert_eq!(trading_session_utc(mk(23)), TradingSession::OffHours);
    }
}
