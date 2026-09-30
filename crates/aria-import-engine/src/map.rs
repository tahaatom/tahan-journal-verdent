//! نگاشت ردیف‌های خام بروکر به فیلدهای کانونی (فاز ۱.۱۵).
//!
//! زمان متاتریدر («2024.01.15 10:30:00») به RFC3339 UTC تبدیل می‌شود؛
//! اعداد با نقطه اعشار و بدون جداکننده هزار پذیرفته می‌شوند.

use crate::error::ImportError;
use crate::model::{DealRole, DealSide, MappedDeal};
use crate::parse::{column_index, parse_csv, parse_mt4_html, row_hash, ParsedCsv, SourceFormat};
use chrono::{NaiveDateTime, Utc};

/// تبدیل زمان متاتریدر به RFC3339 UTC.
/// «2024.01.15 10:30:00» یا «2024.01.15 10:30» — ورودی زمان بروکر
/// بدون منطقه است و به‌صورت UTC ثبت می‌شود (فرض A-033).
pub fn parse_mt_time(s: &str) -> Result<String, ImportError> {
    let t = s.trim();
    if t.is_empty() {
        return Err(ImportError::msg("زمان خالی است"));
    }
    let fmts = ["%Y.%m.%d %H:%M:%S", "%Y.%m.%d %H:%M", "%Y-%m-%d %H:%M:%S", "%Y/%m/%d %H:%M:%S"];
    for f in fmts {
        if let Ok(dt) = NaiveDateTime::parse_from_str(t, f) {
            return Ok(dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        }
    }
    Err(ImportError::msg(format!("قالب زمان نامعتبر: «{t}»")))
}

/// تبدیل عدد اعشاری متاتریدر.
pub fn parse_f64(s: &str) -> Result<f64, ImportError> {
    let t = s.trim().replace(',', "");
    if t.is_empty() {
        return Err(ImportError::msg("عدد خالی است"));
    }
    t.parse::<f64>()
        .map_err(|_| ImportError::msg(format!("عدد نامعتبر: «{}»", s.trim())))
}

/// عدد اختیاری — خانه خالی/ستون ناموجود = مقدار پیش‌فرض؛ مقدار نامعتبر = خطا.
fn opt_num(v: Option<&str>, default: f64) -> Result<f64, ImportError> {
    match v {
        Some(s) if !s.trim().is_empty() => parse_f64(s),
        _ => Ok(default),
    }
}

/// تبدیل حجم — «0.10» یا «0.10 / 0.10» (قالب گزارش MT5).
fn parse_volume(s: &str) -> Result<f64, ImportError> {
    let first = s.split('/').next().unwrap_or(s);
    let v = parse_f64(first)?;
    if v <= 0.0 {
        return Err(ImportError::msg(format!("حجم باید مثبت باشد: «{}»", s.trim())));
    }
    Ok(v)
}

/// تبدیل جهت — buy/sell (MT4) یا direction در/out (MT5).
fn parse_side_and_role(type_col: &str, dir_col: Option<&str>) -> Result<(DealSide, DealRole), ImportError> {
    let side = match type_col.trim().to_ascii_lowercase().as_str() {
        "buy" | "0" => DealSide::Buy,
        "sell" | "1" => DealSide::Sell,
        other => {
            return Err(ImportError::msg(format!(
                "نوع معامله ناشناخته: «{other}» (فقط buy/sell پشتیبانی می‌شود)"
            )));
        }
    };
    let role = match dir_col.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("in") | Some("0") => DealRole::In,
        Some("out") | Some("1") => DealRole::Out,
        Some("in/out") => DealRole::Unknown,
        _ => DealRole::Unknown,
    };
    Ok((side, role))
}

/// تبدیل جهت با فرض نقش ورود — قالب خلاصه MT4 (ستون Direction ندارد).
fn parse_side(type_col: &str) -> Result<DealSide, ImportError> {
    parse_side_and_role(type_col, Some("in")).map(|(side, _)| side)
}

/// نتیجه نگاشت یک فایل.
#[derive(Debug, Default)]
pub struct MapOutcome {
    /// دیل‌های نگاشت‌شده موفق
    pub deals: Vec<MappedDeal>,
    /// خطاهای ردیفی — (اندیس ردیف، پیام فارسی)
    pub errors: Vec<(usize, String)>,
    /// ردیف‌های غیرمعاملاتی رد‌شده (موجودی/اعتبار/خالی)
    pub skipped: usize,
    /// متن خام ردیف‌های ناموفق — (اندیس ردیف، متن خام) برای حفظ ممیزی
    pub failed_rows: Vec<(usize, String)>,
}

/// نگاشت کل فایل به دیل‌ها — خطاهای ردیفی جمع می‌شوند و ردیف‌های
/// غیرمعاملاتی (موجودی/اعتبار) در `skipped` شمرده می‌شوند.
pub fn map_file(format: SourceFormat, text: &str) -> MapOutcome {
    match format {
        SourceFormat::CsvDeals => map_csv(text),
        SourceFormat::HtmlStatement => map_html(text),
    }
}

/// ردیف موجودی/اعتبار یا خالی — معامله نیست.
fn is_non_trade_row(type_col: &str, item: &str, all_empty: bool) -> bool {
    all_empty
        || item.eq_ignore_ascii_case("balance")
        || item.eq_ignore_ascii_case("credit")
        || type_col.eq_ignore_ascii_case("balance")
        || type_col.eq_ignore_ascii_case("credit")
}

fn map_csv(text: &str) -> MapOutcome {
    let mut out = MapOutcome::default();
    let parsed = match parse_csv(text) {
        Ok(p) => p,
        Err(e) => {
            out.errors.push((0, e.to_string()));
            return out;
        }
    };
    let ParsedCsv { headers, rows } = parsed;
    let idx = |name: &str| column_index(&headers, name);
    // سرستون‌های MT5 Deals
    let c_time = idx("time");
    let c_position = idx("position");
    let c_type = idx("type");
    let c_dir = idx("direction");
    let c_volume = idx("volume");
    let c_price = idx("price");
    let c_symbol = idx("symbol");
    let c_commission = idx("commission");
    let c_swap = idx("swap");
    let c_profit = idx("profit");
    let c_order = idx("order");
    let c_deal = idx("deal");
    let c_magic = idx("magic");
    let c_comment = idx("comment");
    // سرستون‌های جایگزین صورت‌حساب MT4 (CSV)
    let c_open_time = idx("open time");
    let c_close_time = idx("close time");
    let c_item = idx("item");
    let c_size = idx("size");
    let c_ticket = idx("ticket");
    let c_taxes = idx("taxes");
    // شناسه پوزیشن: MT5 = Position؛ MT4 = Ticket (هر ردیف خلاصه یک معامله بسته)
    let position_col = c_position.or(c_ticket);
    let is_statement = c_close_time.is_some() || (c_item.is_some() && c_size.is_some());

    for (i, (raw, values)) in rows.into_iter().enumerate() {
        let get = |ci: Option<usize>| ci.and_then(|c| values.get(c)).map(|s| s.as_str());
        let row_no = i + 2; // + سرستون
        // ردیف موجودی/اعتبار (قالب CSV صورت‌حساب MT4) و ردیف خالی — معامله نیست
        let all_empty = values.iter().all(|v| v.trim().is_empty());
        if is_non_trade_row(
            get(c_type).unwrap_or(""),
            get(c_item).or(get(c_symbol)).unwrap_or(""),
            all_empty,
        ) {
            out.skipped += 1;
            continue;
        }
        let mapped = (|| -> Result<MappedDeal, ImportError> {
            let time = parse_mt_time(
                get(c_time)
                    .or(get(c_open_time))
                    .ok_or_else(|| ImportError::msg("ستون Time یافت نشد"))?,
            )?;
            let type_col = get(c_type).ok_or_else(|| ImportError::msg("ستون Type یافت نشد"))?;
            // قالب صورت‌حساب MT4 ستون Direction ندارد؛ هر ردیف خلاصه = اجرای ورود
            let (direction, role) = if is_statement {
                (parse_side(type_col)?, DealRole::In)
            } else {
                parse_side_and_role(type_col, get(c_dir))?
            };
            let close_time = match get(c_close_time).filter(|v| !v.trim().is_empty()) {
                Some(v) => Some(parse_mt_time(v)?),
                None => None,
            };
            let symbol = get(c_symbol)
                .or(get(c_item))
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| ImportError::msg("ستون Symbol خالی است"))?
                .trim()
                .to_uppercase();
            Ok(MappedDeal {
                row_index: row_no,
                raw: raw.clone(),
                payload_hash: row_hash(&raw),
                ticket: get(c_ticket)
                    .or(get(c_deal))
                    .or(get(c_order))
                    .and_then(|v| v.parse::<u64>().ok().map(|_| v.to_string())),
                position_id: get(position_col)
                    .filter(|v| !v.trim().is_empty() && v.trim() != "0")
                    .map(|v| v.trim().to_string()),
                symbol,
                direction,
                role,
                time,
                close_time,
                price: parse_f64(get(c_price).ok_or_else(|| ImportError::msg("ستون Price یافت نشد"))?)?,
                volume: parse_volume(
                    get(c_volume)
                        .or(get(c_size))
                        .ok_or_else(|| ImportError::msg("ستون Volume یافت نشد"))?,
                )?,
                // مالیات صورت‌حساب MT4 در کارمزد تجمیع می‌شود
                commission: opt_num(get(c_commission), 0.0)? + opt_num(get(c_taxes), 0.0)?,
                swap: opt_num(get(c_swap), 0.0)?,
                profit: opt_num(get(c_profit), 0.0)?,
                magic: get(c_magic).filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string()),
                comment: get(c_comment).filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string()),
            })
        })();
        match mapped {
            Ok(d) => out.deals.push(d),
            Err(e) => {
                out.errors.push((row_no, e.to_string()));
                out.failed_rows.push((row_no, raw.clone()));
            }
        }
    }
    out
}

/// یافتن مقدار یک ستون در جفت‌های (نام، مقدار) ردیف HTML — بی‌حساس به بزرگی حرف.
fn get_pair<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn map_html(text: &str) -> MapOutcome {
    let mut out = MapOutcome::default();
    let rows = match parse_mt4_html(text) {
        Ok(r) => r,
        Err(e) => {
            out.errors.push((0, e.to_string()));
            return out;
        }
    };
    for (i, (raw, pairs)) in rows.into_iter().enumerate() {
        let row_no = i + 1;
        // ردیف‌های موجودی/اعتبار صورت‌حساب — معامله نیستند
        // (در صورت‌حساب MT4 ستون Type «balance/credit» می‌شود)
        let type_col = get_pair(&pairs, "type").unwrap_or("");
        let item = get_pair(&pairs, "item").unwrap_or("");
        let all_empty = pairs.iter().all(|(_, v)| v.trim().is_empty());
        if is_non_trade_row(type_col, item, all_empty) || type_col.is_empty() {
            out.skipped += 1;
            continue;
        }
        let mapped = (|| -> Result<MappedDeal, ImportError> {
            let open_time = parse_mt_time(get_pair(&pairs, "open time").ok_or_else(|| ImportError::msg("ستون Open Time یافت نشد"))?)?;
            let close_time_raw = get_pair(&pairs, "close time").unwrap_or("");
            let close_time = if close_time_raw.trim().is_empty() {
                None
            } else {
                Some(parse_mt_time(close_time_raw)?)
            };
            let (direction, _role) = parse_side_and_role(get_pair(&pairs, "type").ok_or_else(|| ImportError::msg("ستون Type یافت نشد"))?, None)?;
            Ok(MappedDeal {
                row_index: row_no,
                raw: raw.clone(),
                payload_hash: row_hash(&raw),
                ticket: get_pair(&pairs, "ticket").and_then(|v| v.parse::<u64>().ok().map(|_| v.to_string())),
                position_id: get_pair(&pairs, "ticket").and_then(|v| v.parse::<u64>().ok().map(|_| v.to_string())),
                symbol: get_pair(&pairs, "item")
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| ImportError::msg("ستون Item خالی است"))?
                    .trim()
                    .to_uppercase(),
                direction,
                // ردیف خلاصه MT4 اجرای بازشدن معاملهٔ بسته‌شده را نمایندگی می‌کند
                role: DealRole::In,
                time: open_time.clone(),
                close_time,
                price: parse_f64(get_pair(&pairs, "price").ok_or_else(|| ImportError::msg("ستون Price یافت نشد"))?)?,
                volume: parse_volume(get_pair(&pairs, "size").ok_or_else(|| ImportError::msg("ستون Size یافت نشد"))?)?,
                commission: opt_num(get_pair(&pairs, "commission"), 0.0)?,
                swap: opt_num(get_pair(&pairs, "swap"), 0.0)?,
                profit: opt_num(get_pair(&pairs, "profit"), 0.0)?,
                magic: None,
                comment: None,
            })
        })();
        match mapped {
            Ok(d) => out.deals.push(d),
            Err(e) => {
                out.errors.push((row_no, e.to_string()));
                out.failed_rows.push((row_no, raw.clone()));
            }
        }
    }
    out
}

/// زمان کنونی ISO (برای تست‌ها و service).
pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MT5_CSV: &str = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
        2024.01.15 10:30:00,123456,buy,in,0.10,2035.50,789,0.00,0.00,0.00,XAUUSD,\n\
        2024.01.15 12:00:00,123456,sell,out,0.10,2040.00,790,-0.50,0.20,45.00,XAUUSD,take profit\n";

    #[test]
    fn parses_mt_time_to_rfc3339() {
        assert_eq!(parse_mt_time("2024.01.15 10:30:00").unwrap(), "2024-01-15T10:30:00Z");
        assert_eq!(parse_mt_time("2024.01.15 10:30").unwrap(), "2024-01-15T10:30:00Z");
        let err = parse_mt_time("15/01/2024").unwrap_err();
        assert!(err.to_string().contains("نامعتبر"));
    }

    #[test]
    fn parses_numbers_and_volume() {
        assert!((parse_f64("-0.50").unwrap() + 0.5).abs() < 1e-9);
        assert!((parse_volume("0.10 / 0.10").unwrap() - 0.10).abs() < 1e-9);
        assert!(parse_volume("0").is_err());
        assert!(parse_f64("abc").is_err());
    }

    #[test]
    fn maps_mt5_deals_csv() {
        let out = map_file(SourceFormat::CsvDeals, MT5_CSV);
        assert!(out.errors.is_empty());
        assert_eq!(out.skipped, 0);
        let deals = out.deals;
        assert_eq!(deals.len(), 2);
        let first = &deals[0];
        assert_eq!(first.symbol, "XAUUSD");
        assert_eq!(first.direction, DealSide::Buy);
        assert_eq!(first.role, DealRole::In);
        assert_eq!(first.position_id.as_deref(), Some("123456"));
        assert_eq!(first.time, "2024-01-15T10:30:00Z");
        assert!((first.price - 2035.50).abs() < 1e-9);
        let second = &deals[1];
        assert_eq!(second.role, DealRole::Out);
        assert!((second.profit - 45.0).abs() < 1e-9);
        assert!((second.commission + 0.5).abs() < 1e-9);
        assert_eq!(second.comment.as_deref(), Some("take profit"));
    }

    #[test]
    fn csv_row_errors_are_collected_with_row_numbers() {
        let text = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
            2024.01.15 10:30:00,1,buy,in,0.10,2035.50,1,0,0,0,XAUUSD,\n\
            bad-line-beyond-fix,2,sell,out,0.10,2040.00,2,0,0,0,XAUUSD,\n\
            2024.01.15 11:00:00,3,buy,in,abc,2035.50,3,0,0,0,XAUUSD,\n";
        let out = map_file(SourceFormat::CsvDeals, text);
        assert_eq!(out.deals.len(), 1);
        assert_eq!(out.errors.len(), 2);
        assert_eq!(out.errors[0].0, 3, "row numbering includes the header");
        assert!(
            out.errors[1].1.contains("نامعتبر"),
            "bad volume is a Persian error"
        );
    }

    #[test]
    fn csv_balance_and_empty_rows_are_skipped() {
        // قالب CSV صورت‌حساب MT4: ردیف‌های balance/credit و ردیف خالی معامله نیستند
        let text = "Ticket,Open Time,Type,Size,Item,Price,Profit\n\
            1,2024.01.15 10:30,buy,0.10,XAUUSD,2035.50,45.00\n\
            2,2024.01.16 00:00,balance,,,0,1000.00\n\
            ,,,,,,\n\
            3,2024.01.17 00:00,credit,,,0,500.00\n";
        let out = map_file(SourceFormat::CsvDeals, text);
        assert_eq!(out.deals.len(), 1, "only the real trade is mapped");
        assert_eq!(out.errors.len(), 0, "skipped rows are not errors");
        assert_eq!(out.skipped, 3);
    }

    #[test]
    fn maps_mt4_statement_html() {
        let html = r#"<html><body><table>
            <tr><td>Ticket</td><td>Open Time</td><td>Type</td><td>Size</td><td>Item</td><td>Price</td><td>Close Time</td><td>Commission</td><td>Taxes</td><td>Swap</td><td>Profit</td></tr>
            <tr><td>771001</td><td>2024.01.15 10:30</td><td>buy</td><td>0.10</td><td>xauusd</td><td>2035.50</td><td>2024.01.15 12:00</td><td>-0.50</td><td>0.00</td><td>0.20</td><td>45.00</td></tr>
            <tr><td></td><td>2024.01.16 00:00</td><td>balance</td><td></td><td></td><td>0</td><td></td><td>0.00</td><td>0.00</td><td>0.00</td><td>1000.00</td></tr>
        </table></body></html>"#;
        let out = map_file(SourceFormat::HtmlStatement, html);
        assert!(out.errors.is_empty());
        assert_eq!(out.deals.len(), 1, "balance rows are skipped");
        assert_eq!(out.skipped, 1);
        let d = &out.deals[0];
        assert_eq!(d.ticket.as_deref(), Some("771001"));
        assert_eq!(d.symbol, "XAUUSD");
        assert_eq!(d.time, "2024-01-15T10:30:00Z");
        assert_eq!(d.close_time.as_deref(), Some("2024-01-15T12:00:00Z"));
        assert!((d.profit - 45.0).abs() < 1e-9);
    }

    #[test]
    fn row_hash_is_stable_and_distinct() {
        assert_eq!(row_hash("a,b"), row_hash("a,b"));
        assert_ne!(row_hash("a,b"), row_hash("a, b"));
    }
}
