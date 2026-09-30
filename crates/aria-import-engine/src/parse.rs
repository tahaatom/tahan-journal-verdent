//! تجزیه فایل‌های خروجی متاتریدر — CSV/TSV معاملات MT5 و HTML صورت‌حساب MT4.
//!
//! CSV با کرت `csv` (تشخیص جدول‌بند) و HTML با تجزیه‌گر امن `scraper`
//! (html5ever) — هرگز از regex برای HTML استفاده نمی‌شود.

use crate::error::ImportError;
pub use crate::model::SourceFormat;
use scraper::{Html, Selector};

/// تشخیص قالب فایل از نام و محتوا.
pub fn detect_format(file_name: &str, text: &str) -> SourceFormat {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        return SourceFormat::HtmlStatement;
    }
    // محتوای HTML بدون پسوند — تگ جدول یا doctype
    let head = text.get(..text.len().min(2048)).unwrap_or(text).to_ascii_lowercase();
    if head.contains("<html") || head.contains("<table") || head.contains("<!doctype") {
        return SourceFormat::HtmlStatement;
    }
    SourceFormat::CsvDeals
}

/// هش blake3 متن ردیف — کلید تشخیص تکرار.
pub fn row_hash(raw: &str) -> String {
    blake3::hash(raw.as_bytes()).to_hex().to_string()
}

/// یک جدول‌بند تشخیص می‌دهد: اگر خط سرستون تِب داشته باشد TSV است.
fn sniff_delimiter(header_line: &str) -> u8 {
    if header_line.contains('\t') {
        b'\t'
    } else {
        b','
    }
}

/// نتیجه تجزیه CSV — سرستون‌ها + ردیف‌های خام (متن خط اصلی).
pub struct ParsedCsv {
    pub headers: Vec<String>,
    /// (متن خام خط، مقادیر ستون‌ها)
    pub rows: Vec<(String, Vec<String>)>,
}

/// تجزیه CSV/TSV با حفظ متن خام هر خط.
pub fn parse_csv(text: &str) -> Result<ParsedCsv, ImportError> {
    let mut lines = text.lines();
    let header_line = lines
        .next()
        .ok_or_else(|| ImportError::msg("فایل CSV سرستون ندارد"))?
        .trim_start_matches('\u{FEFF}')
        .to_string();
    let delim = sniff_delimiter(&header_line);
    let headers = split_line(&header_line, delim);

    let mut rows = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let raw = line.to_string();
        let values = split_line(line, delim);
        rows.push((raw, values));
    }
    Ok(ParsedCsv { headers, rows })
}

/// تقسیم یک خط CSV با رعایت نقل‌قول (بدون کرت کامل برای حفظ متن خام).
fn split_line(line: &str, delim: u8) -> Vec<String> {
    let d = delim as char;
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.is_empty() {
            in_quotes = true;
        } else if c == d {
            out.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// یافتن اندیس ستون بر اساس نام (بی‌حساس به بزرگی حرف).
pub fn column_index(headers: &[String], name: &str) -> Option<usize> {
    let want = name.to_ascii_lowercase();
    headers
        .iter()
        .position(|h| h.trim().trim_matches('"').to_ascii_lowercase() == want)
}

/// ردیف‌های جدول HTML — (متن خام، جفت‌های (نام سرستون، مقدار))
pub type HtmlRows = Vec<(String, Vec<(String, String)>)>;

/// استخراج ردیف‌های جدول معاملات از HTML صورت‌حساب MT4 با تجزیه‌گر امن.
pub fn parse_mt4_html(text: &str) -> Result<HtmlRows, ImportError> {
    let doc = Html::parse_document(text);
    let tr_sel = Selector::parse("tr").map_err(|e| ImportError::msg(format!("خطای تجزیه HTML: {e}")))?;
    let td_sel = Selector::parse("td,th").map_err(|e| ImportError::msg(format!("خطای تجزیه HTML: {e}")))?;

    let mut tables: Vec<Vec<Vec<String>>> = Vec::new();
    for tr in doc.select(&tr_sel) {
        let cells: Vec<String> = tr
            .select(&td_sel)
            .map(|c| c.text().collect::<String>().trim().to_string())
            .collect();
        if cells.is_empty() {
            continue;
        }
        // ردیف سرستون شامل «Ticket» ابتدای جدول جدید است
        if cells
            .iter()
            .any(|c| c.eq_ignore_ascii_case("ticket"))
        {
            tables.push(vec![cells]);
        } else if let Some(last) = tables.last_mut() {
            last.push(cells);
        }
    }
    let first = tables
        .first()
        .ok_or_else(|| ImportError::msg("جدول معاملات در فایل HTML یافت نشد"))?;

    let headers = first[0].clone();
    let mut rows = Vec::new();
    for cells in &first[1..] {
        let pairs: Vec<(String, String)> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.clone(), cells.get(i).cloned().unwrap_or_default()))
            .collect();
        rows.push((headers.join(" | "), pairs));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_csv_html_and_content_sniffing() {
        assert_eq!(detect_format("h.csv", "a,b"), SourceFormat::CsvDeals);
        assert_eq!(detect_format("statement.htm", "<table>"), SourceFormat::HtmlStatement);
        assert_eq!(detect_format("report.txt", "<html><body>"), SourceFormat::HtmlStatement);
        assert_eq!(detect_format("deals.tsv", "Time\tDeal"), SourceFormat::CsvDeals);
    }

    #[test]
    fn parses_tsv_with_quoted_fields() {
        let text = "Time\tPosition\tType\tDirection\t\"Volume, Lots\"\tPrice\r\n\
                    2024.01.15 10:30:00\t123\tbuy\tin\t0.10\t2035.50\r\n\
                    2024.01.15 12:00:00\t123\tsell\tout\t0.10\t2040.00\r\n";
        let parsed = parse_csv(text).unwrap();
        assert_eq!(parsed.headers.len(), 6);
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.rows[0].1[4], "0.10");
        // حفظ متن خام خط (بدون \r)
        assert!(parsed.rows[0].0.ends_with("2035.50"));
    }

    #[test]
    fn column_index_is_case_insensitive() {
        let h = vec!["Time".into(), " POSITION ".into(), "Type".into()];
        assert_eq!(column_index(&h, "position"), Some(1));
        assert_eq!(column_index(&h, "missing"), None);
    }

    #[test]
    fn parses_mt4_statement_table() {
        let html = r#"<html><body><table>
            <tr><td>Ticket</td><td>Open Time</td><td>Type</td><td>Size</td><td>Item</td><td>Price</td><td>Close Time</td><td>Commission</td><td>Taxes</td><td>Swap</td><td>Profit</td></tr>
            <tr><td>771001</td><td>2024.01.15 10:30</td><td>buy</td><td>0.10</td><td>XAUUSD</td><td>2035.50</td><td>2024.01.15 12:00</td><td>-0.50</td><td>0.00</td><td>0.20</td><td>45.00</td></tr>
            <tr><td></td><td colspan="13">Balance</td></tr>
        </table></body></html>"#;
        let rows = parse_mt4_html(html).unwrap();
        assert_eq!(rows.len(), 2, "balance row is kept for the mapper to skip");
        let map = &rows[0].1;
        let ticket = map.iter().find(|(k, _)| k == "Ticket").unwrap();
        assert_eq!(ticket.1, "771001");
    }

    #[test]
    fn html_without_table_errors() {
        let err = parse_mt4_html("<html><body>no table</body></html>").unwrap_err();
        assert_eq!(err.code(), 1500);
    }
}
