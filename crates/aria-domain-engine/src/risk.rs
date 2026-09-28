//! سرویس محاسبه R — قطعی و تست‌شده.
//!
//! مرجع قرارداد: `docs/contracts/r-calculation-contract.md`
//!
//! قواعد قطعی نسخه ۱:
//! 1. ورود چندگانه → میانگین وزنی حجم.
//! 2. مبنای ریسک همیشه حد ضرر اولیه است؛ جابجایی SL بعداً R را تغییر نمی‌دهد
//!    (محاسبه از `initial_stop_loss` معامله انجام می‌شود، نه SL پاها).
//! 3. بدون SL و بدون ریسک دستی → `no_stop_loss` و R خودکار ممنوع.
//! 4. ریسک دستی (`manual_risk`) → `manual_risk` و مبلغ ریسک = مقدار دستی.
//! 5. کمیسیون و سوآپ از RealizedPnL کسر می‌شوند.
//! 6. خروج چندگانه → جمع PnL خالص همه خروج‌ها ÷ ریسک اولیه = TradeR.
//! 7. اجرای `needs_assignment` در PnL و R وارد نمی‌شود (وضعیت کل معامله هم `needs_assignment`).

use serde::{Deserialize, Serialize};

/// وضعیت محاسبه ریسک.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCalculationStatus {
    /// هنوز داده کافی نیست (مثلاً بدون پای ورود اجراشده)
    Pending,
    /// با موفقیت محاسب شد
    Calculated,
    /// بدون حد ضرر — محاسبه خودکار ممنوع
    NoStopLoss,
    /// ریسک دستی کاربر
    ManualRisk,
    /// اجرای تخصیص‌نیافته وجود دارد
    NeedsAssignment,
}

impl RiskCalculationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Calculated => "calculated",
            Self::NoStopLoss => "no_stop_loss",
            Self::ManualRisk => "manual_risk",
            Self::NeedsAssignment => "needs_assignment",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => Self::Pending,
            "calculated" => Self::Calculated,
            "no_stop_loss" => Self::NoStopLoss,
            "manual_risk" => Self::ManualRisk,
            "needs_assignment" => Self::NeedsAssignment,
            _ => return None,
        })
    }
}

/// مبنای ریسک.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskBasis {
    InitialStopLoss,
    ManualRisk,
}

impl RiskBasis {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InitialStopLoss => "initial_stop_loss",
            Self::ManualRisk => "manual_risk",
        }
    }
}

/// ورودی پای ورود برای محاسبه (فقط مقادیر لازم — جدا از مدل ذخیره‌سازی).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryLegInput {
    /// قیمت اجراشده (قیمت پیش‌نمایشِ planned در محاسبه R واقعی به‌کار نمی‌رود)
    pub executed_price: f64,
    pub volume: f64,
}

/// ورودی پای خروج برای محاسبه.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExitLegInput {
    pub executed_price: f64,
    pub volume: f64,
}

/// خروجی کامل محاسبه ریسک برای یک معامله.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RiskComputation {
    /// نسبت ریسک به هدف بر اساس داده پیش‌نمایش (ممکن است None باشد)
    pub planned_r: Option<f64>,
    /// |میانگین ورود − حد ضرر اولیه|
    pub initial_risk_price_distance: Option<f64>,
    /// فاصله ریسکی × حجم کل × ارزش واحد، یا مقدار ریسک دستی
    pub initial_risk_amount: Option<f64>,
    /// سود/زیان محقق‌شده خالص (پس از کسر کمیسیون و سوآپ)
    pub realized_pnl: Option<f64>,
    /// RealizedPnL ÷ InitialRiskAmount
    pub realized_r: Option<f64>,
    /// R هر پای خروج به‌تنهایی (فقط گزارشی)
    pub exit_leg_r: Vec<f64>,
    /// R کل معامله — بر مبنای نتیجه خالص کل
    pub trade_r: Option<f64>,
    pub status: RiskCalculationStatus,
    pub basis: Option<RiskBasis>,
}

/// میانگین وزنی حجمی ورودها.
pub fn weighted_average_entry(legs: &[EntryLegInput]) -> Option<f64> {
    if legs.is_empty() {
        return None;
    }
    let mut value = 0.0;
    let mut volume = 0.0;
    for l in legs {
        value += l.executed_price * l.volume;
        volume += l.volume;
    }
    if volume <= 0.0 || !value.is_finite() || !volume.is_finite() {
        return None;
    }
    Some(value / volume)
}

/// PlannedR — نسبت فاصله ریسک به فاصله هدف بر پایه داده پیش‌نمایش فرم.
///
/// مخرج/صورت از `planned_price` پای ورود، `initial_stop_loss` و `take_profit` معامله.
pub fn planned_r(
    planned_entry: Option<f64>,
    initial_stop_loss: Option<f64>,
    take_profit: Option<f64>,
    direction_sign: f64,
) -> Option<f64> {
    let (entry, sl, tp) = match (planned_entry, initial_stop_loss, take_profit) {
        (Some(e), Some(s), Some(t)) => (e, s, t),
        _ => return None,
    };
    let risk = (entry - sl) * direction_sign;
    let reward = (tp - entry) * direction_sign;
    if risk <= 0.0 || !risk.is_finite() || !reward.is_finite() {
        return None;
    }
    Some(reward / risk)
}

/// فاصله قیمت ریسک اولیه: |میانگین ورود − حد ضرر|.
pub fn initial_risk_price_distance(avg_entry: Option<f64>, initial_stop_loss: Option<f64>) -> Option<f64> {
    let (e, s) = match (avg_entry, initial_stop_loss) {
        (Some(e), Some(s)) => (e, s),
        _ => return None,
    };
    let d = (e - s).abs();
    if !d.is_finite() {
        return None;
    }
    Some(d)
}

/// PnL یک پای خروج نسبت به میانگین ورود (بدون هزینه‌ها).
pub fn exit_leg_pnl(
    direction_sign: f64,
    avg_entry: f64,
    contract_size: f64,
    leg: &ExitLegInput,
) -> f64 {
    direction_sign * (leg.executed_price - avg_entry) * leg.volume * contract_size
}

/// محاسبه کامل R یک معامله.
///
// `has_unassigned_executions`: اگر درست باشد، خروجی `needs_assignment` است
// و هیچ مقدار PnL/R محاسبه نمی‌شود (قاعده ۷ قرارداد).
#[allow(clippy::too_many_arguments)] // تابع ریاضی قطعی با ورودی‌های مستقل و مستندشده
pub fn compute_trade_risk(
    direction_sign: f64,
    contract_size: f64,
    commission: f64,
    swap: f64,
    initial_stop_loss: Option<f64>,
    manual_risk: Option<f64>,
    take_profit: Option<f64>,
    planned_entry: Option<f64>,
    entry_legs: &[EntryLegInput],
    exit_legs: &[ExitLegInput],
    has_unassigned_executions: bool,
) -> RiskComputation {
    if has_unassigned_executions {
        return RiskComputation {
            planned_r: None,
            initial_risk_price_distance: None,
            initial_risk_amount: None,
            realized_pnl: None,
            realized_r: None,
            exit_leg_r: vec![],
            trade_r: None,
            status: RiskCalculationStatus::NeedsAssignment,
            basis: None,
        };
    }

    // مبنای ریسک: ریسک دستی اولویت دارد؛ سپس SL اولیه؛ در غیر این صورت محاسبه ممنوع
    let (status, basis): (RiskCalculationStatus, Option<RiskBasis>) = if manual_risk.is_some() {
        (RiskCalculationStatus::ManualRisk, Some(RiskBasis::ManualRisk))
    } else if initial_stop_loss.is_some() {
        (RiskCalculationStatus::Calculated, Some(RiskBasis::InitialStopLoss))
    } else {
        (RiskCalculationStatus::NoStopLoss, None)
    };

    let avg_entry = weighted_average_entry(entry_legs);
    let price_distance = initial_risk_price_distance(avg_entry, initial_stop_loss);

    let initial_risk_amount = match (basis, manual_risk) {
        (Some(RiskBasis::ManualRisk), Some(m)) => Some(m),
        (Some(RiskBasis::InitialStopLoss), _) => {
            price_distance.and_then(|d| {
                let vol: f64 = entry_legs.iter().map(|l| l.volume).sum();
                let amount = d * vol * contract_size;
                if amount.is_finite() {
                    Some(amount)
                } else {
                    None
                }
            })
        }
        _ => None,
    };

    // PnL و R فقط با داشتن مبنای ریسک و میانگین ورود محاسبه می‌شوند
    let mut realized_pnl = None;
    let mut realized_r = None;
    let mut trade_r = None;
    let mut exit_leg_r = Vec::new();

    if let (Some(avg), Some(risk)) = (avg_entry, initial_risk_amount) {
        if risk > 0.0 && risk.is_finite() {
            let gross: f64 = exit_legs
                .iter()
                .map(|l| exit_leg_pnl(direction_sign, avg, contract_size, l))
                .sum();
            if gross.is_finite() {
                let net = gross - commission - swap;
                realized_pnl = Some(net);
                realized_r = Some(net / risk);
                trade_r = realized_r;
                exit_leg_r = exit_legs
                    .iter()
                    .map(|l| exit_leg_pnl(direction_sign, avg, contract_size, l) / risk)
                    .collect();
            }
        }
    }

    RiskComputation {
        planned_r: planned_r(planned_entry, initial_stop_loss, take_profit, direction_sign),
        initial_risk_price_distance: price_distance,
        initial_risk_amount,
        realized_pnl,
        realized_r,
        exit_leg_r,
        trade_r,
        status,
        basis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(price: f64, volume: f64) -> EntryLegInput {
        EntryLegInput { executed_price: price, volume }
    }

    fn exit(price: f64, volume: f64) -> ExitLegInput {
        ExitLegInput { executed_price: price, volume }
    }

    // ---------- تک ورود / تک خروج ----------

    #[test]
    fn single_entry_single_exit_buy() {
        // ورود ۱۰۰ با حجم ۱، SL=۹۰ → ریسک = ۱۰×۱×۱=۱۰؛ خروج ۱۱۵ → +۱۵ → R=۱.۵
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            Some(90.0), None, None, None,
            &[entry(100.0, 1.0)],
            &[exit(115.0, 1.0)],
            false,
        );
        assert_eq!(r.status, RiskCalculationStatus::Calculated);
        assert_eq!(r.basis, Some(RiskBasis::InitialStopLoss));
        assert_eq!(r.initial_risk_price_distance, Some(10.0));
        assert_eq!(r.initial_risk_amount, Some(10.0));
        assert_eq!(r.realized_pnl, Some(15.0));
        assert_eq!(r.realized_r, Some(1.5));
        assert_eq!(r.trade_r, Some(1.5));
        assert_eq!(r.exit_leg_r, vec![1.5]);
    }

    #[test]
    fn single_entry_single_exit_sell() {
        // فروش ۱۰۰، SL=۱۰۵ → ریسک ۵؛ خروج ۹۵ → +۵ → R=۱
        let r = compute_trade_risk(
            -1.0, 1.0, 0.0, 0.0,
            Some(105.0), None, None, None,
            &[entry(100.0, 2.0)],
            &[exit(95.0, 2.0)],
            false,
        );
        assert_eq!(r.initial_risk_amount, Some(10.0)); // ۵×۲×۱
        assert_eq!(r.realized_pnl, Some(10.0));
        assert_eq!(r.realized_r, Some(1.0));
    }

    // ---------- میانگین وزنی چند ورود ----------

    #[test]
    fn multi_entry_weighted_average() {
        // ورود ۱۰۰×۱ و ۱۱۰×۳ → میانگین = (۱۰۰+۳۳۰)/۴ = ۱۰۷.۵
        let avg = weighted_average_entry(&[entry(100.0, 1.0), entry(110.0, 3.0)]).unwrap();
        assert_eq!(avg, 107.5);
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            Some(102.5), None, None, None,
            &[entry(100.0, 1.0), entry(110.0, 3.0)],
            &[exit(112.5, 4.0)],
            false,
        );
        // ریسک = ۵×۴ = ۲۰؛ سود = ۵×۴ = ۲۰ → R=۱
        assert_eq!(r.initial_risk_amount, Some(20.0));
        assert_eq!(r.realized_pnl, Some(20.0));
        assert_eq!(r.realized_r, Some(1.0));
    }

    // ---------- چند خروج ----------

    #[test]
    fn multi_exit_summed_then_divided() {
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            Some(90.0), None, None, None,
            &[entry(100.0, 1.0)],
            &[exit(110.0, 0.5), exit(120.0, 0.5)],
            false,
        );
        // سود = ۱۰×۰.۵ + ۲۰×۰.۵ = ۱۵ → R=۱.۵
        assert_eq!(r.realized_pnl, Some(15.0));
        assert_eq!(r.realized_r, Some(1.5));
        assert_eq!(r.exit_leg_r, vec![0.5, 1.0]);
    }

    // ---------- بدون حد ضرر ----------

    #[test]
    fn no_stop_loss_forbids_automatic_r() {
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            None, None, None, None,
            &[entry(100.0, 1.0)],
            &[exit(110.0, 1.0)],
            false,
        );
        assert_eq!(r.status, RiskCalculationStatus::NoStopLoss);
        assert_eq!(r.basis, None);
        assert_eq!(r.initial_risk_amount, None);
        assert_eq!(r.realized_r, None);
        assert_eq!(r.realized_pnl, None);
        assert_eq!(r.trade_r, None);
    }

    // ---------- ریسک دستی ----------

    #[test]
    fn manual_risk_overrides_basis() {
        // SL وجود دارد ولی ریسک دستی ۲۵ وارد شده → مبنای ریسک دستی
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            Some(90.0), Some(25.0), None, None,
            &[entry(100.0, 1.0)],
            &[exit(115.0, 1.0)],
            false,
        );
        assert_eq!(r.status, RiskCalculationStatus::ManualRisk);
        assert_eq!(r.basis, Some(RiskBasis::ManualRisk));
        // فاصله قیمت همچنان گزارش می‌شود ولی مبلغ ریسک دستی است
        assert_eq!(r.initial_risk_price_distance, Some(10.0));
        assert_eq!(r.initial_risk_amount, Some(25.0));
        assert_eq!(r.realized_r, Some(0.6));
        // بدون SL هم ریسک دستی کافی است
        let r2 = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            None, Some(20.0), None, None,
            &[entry(100.0, 1.0)],
            &[exit(110.0, 1.0)],
            false,
        );
        assert_eq!(r2.status, RiskCalculationStatus::ManualRisk);
        assert_eq!(r2.realized_r, Some(0.5));
    }

    // ---------- کمیسیون و سوآپ ----------

    #[test]
    fn commission_and_swap_deducted() {
        let r = compute_trade_risk(
            1.0, 1.0, 2.0, 1.0,
            Some(90.0), None, None, None,
            &[entry(100.0, 1.0)],
            &[exit(115.0, 1.0)],
            false,
        );
        assert_eq!(r.realized_pnl, Some(12.0)); // ۱۵ − ۲ − ۱
        assert_eq!(r.realized_r, Some(1.2));
    }

    // ---------- اجرای تخصیص‌نیافته ----------

    #[test]
    fn unassigned_execution_excludes_trade_from_pnl_and_r() {
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            Some(90.0), None, None, None,
            &[entry(100.0, 1.0)],
            &[exit(115.0, 1.0)],
            true,
        );
        assert_eq!(r.status, RiskCalculationStatus::NeedsAssignment);
        assert_eq!(r.realized_pnl, None);
        assert_eq!(r.realized_r, None);
        assert_eq!(r.trade_r, None);
        assert!(r.exit_leg_r.is_empty());
    }

    // ---------- PlannedR ----------

    #[test]
    fn planned_r_two_to_one() {
        // ورود برنامه‌ریزی ۱۰۰، SL=۹۵، TP=۱۱۰ → R برنامه = ۱۰/۵ = ۲
        let pr = planned_r(Some(100.0), Some(95.0), Some(110.0), 1.0);
        assert_eq!(pr, Some(2.0));
        // ناقص → None
        assert_eq!(planned_r(Some(100.0), None, Some(110.0), 1.0), None);
        // جهت نادرست (SL بالای ورود در خرید) → None
        assert_eq!(planned_r(Some(100.0), Some(105.0), Some(110.0), 1.0), None);
    }

    #[test]
    fn no_entry_legs_yields_no_amounts() {
        let r = compute_trade_risk(
            1.0, 1.0, 0.0, 0.0,
            Some(90.0), None, None, None,
            &[],
            &[],
            false,
        );
        assert_eq!(r.initial_risk_price_distance, None);
        // بدون پای ورود، میانگین ورود نامشخص است → PnL و R محاسبه نمی‌شوند
        assert_eq!(r.realized_pnl, None);
        assert_eq!(r.realized_r, None);
        assert_eq!(r.status, RiskCalculationStatus::Calculated);
    }

    #[test]
    fn status_roundtrip() {
        for s in ["pending", "calculated", "no_stop_loss", "manual_risk", "needs_assignment"] {
            assert_eq!(RiskCalculationStatus::parse(s).unwrap().as_str(), s);
        }
        assert_eq!(RiskCalculationStatus::parse("x"), None);
    }
}
