//! مدل‌های تجمیع و آمار پرس‌وجو.

use serde::{Deserialize, Serialize};

/// آمار مرکبی از یک پرس‌وجوی تجمیعی.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoreStats {
    /// کل معاملات منطبق (شامل باز و لغوشده)
    pub total_trades: i64,
    /// معاملات بسته‌شده با PnL قطعی‌شده
    pub closed_trades: i64,
    pub wins: i64,
    pub losses: i64,
    pub breakevens: i64,
    /// سود بسته‌ها ÷ کل بسته‌ها (درصد ۰ تا ۱۰۰)
    pub win_rate: Option<f64>,
    /// میانگین R محقق‌شده معاملات بسته
    pub avg_r: Option<f64>,
    /// جمع PnL تحقق‌یافته
    pub total_pnl: Option<f64>,
    /// حداکثر افت سرمایه پایه (بیشینه «قله − افت»)
    pub max_drawdown: Option<f64>,
}

/// یک نقطه منحنی سرمایه.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// روز (YYYY-MM-DD) به مبنای زمان بستن معامله
    pub date: String,
    pub cumulative_pnl: f64,
}

/// بُعد گروه‌بندی عملکرد.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "dim", content = "field", rename_all = "snake_case")]
pub enum Dimension {
    Symbol,
    Strategy,
    Timeframe,
    Session,
    /// روز هفته بر مبنای تقویم ذخیره‌شده (۰=یکشنبه … ۶=شنبه در strftime)
    Weekday,
    /// فیلد سفارشی با کلید فنی
    CustomField(String),
}

/// آمار یک گروه از عملکرد.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupStat {
    /// کلید گروه (مقدار خام یا شماره روز هفته)
    pub key: String,
    /// برچسب فارسی نمایشی
    pub label: String,
    pub trades: i64,
    pub wins: i64,
    pub win_rate: Option<f64>,
    pub total_pnl: Option<f64>,
    pub avg_r: Option<f64>,
    /// میانگین مقدار فیلد سفارشی — فقط برای فیلدهای عددی (integer/decimal/rating)
    pub avg_custom_value: Option<f64>,
}

/// یک خانه نقشه حرارتی زمان — تجمیع معاملات بسته در ساعتِ روز هفته (فاز ۱.۱۴).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeatCell {
    /// روز هفته (۰=یکشنبه … ۶=شنبه — خروجی strftime '%w')
    pub weekday: i64,
    /// ساعت روز (۰ تا ۲۳ — خروجی strftime '%H')
    pub hour: i64,
    pub trades: i64,
    pub total_pnl: Option<f64>,
}

/// برچسب فارسی روز هفته؛ ورودی خروجی strftime('%w') است.
pub fn weekday_label(w: &str) -> String {
    match w {
        "0" => "یکشنبه".into(),
        "1" => "دوشنبه".into(),
        "2" => "سه‌شنبه".into(),
        "3" => "چهارشنبه".into(),
        "4" => "پنجشنبه".into(),
        "5" => "جمعه".into(),
        "6" => "شنبه".into(),
        other => other.to_string(),
    }
}

/// محاسبه حداکثر افت سرمایه و منحنی سرمایه از PnL روزانه مرتب‌شده.
pub fn fold_equity(daily: &[(String, f64)]) -> (Vec<EquityPoint>, Option<f64>) {
    let mut points = Vec::with_capacity(daily.len());
    let mut cumulative = 0.0_f64;
    let mut peak = 0.0_f64;
    let mut max_dd: Option<f64> = None;
    for (date, pnl) in daily {
        cumulative += pnl;
        if cumulative > peak {
            peak = cumulative;
        }
        let dd = peak - cumulative;
        max_dd = Some(match max_dd {
            Some(m) if m >= dd => m,
            _ => dd,
        });
        points.push(EquityPoint {
            date: date.clone(),
            cumulative_pnl: cumulative,
        });
    }
    (points, max_dd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_labels_are_persian() {
        assert_eq!(weekday_label("6"), "شنبه");
        assert_eq!(weekday_label("0"), "یکشنبه");
        assert_eq!(weekday_label("5"), "جمعه");
        assert_eq!(weekday_label("9"), "9");
    }

    #[test]
    fn fold_equity_tracks_drawdown() {
        // روزها: +۱۰۰، −۲۵۰، +۵۰ → مجموع −۱۰۰
        let daily = vec![
            ("2026-01-01".to_string(), 100.0),
            ("2026-01-02".to_string(), -250.0),
            ("2026-01-03".to_string(), 50.0),
        ];
        let (points, dd) = fold_equity(&daily);
        assert_eq!(points.len(), 3);
        assert!((points[1].cumulative_pnl - (-150.0)).abs() < 1e-9);
        assert!((points[2].cumulative_pnl - (-100.0)).abs() < 1e-9);
        // قله ۱۰۰ → کف −۱۵۰ → افت ۲۵۰
        assert!((dd.unwrap() - 250.0).abs() < 1e-9);
    }

    #[test]
    fn fold_equity_empty_has_no_drawdown() {
        let (points, dd) = fold_equity(&[]);
        assert!(points.is_empty());
        assert!(dd.is_none());
    }

    #[test]
    fn fold_equity_monotonic_profit_has_zero_drawdown() {
        let daily = vec![
            ("2026-01-01".to_string(), 50.0),
            ("2026-01-02".to_string(), 30.0),
        ];
        let (_, dd) = fold_equity(&daily);
        assert!((dd.unwrap()).abs() < 1e-9);
    }
}
