//! نسخه‌گذاری معنایی (semver) و بازه‌های سازگاری — پیاده‌سازی داخلی بدون وابستگی.
//!
//! نسخه پشتیبانی‌شده: `MAJOR.MINOR.PATCH` با پیش‌انتشار اختیاری `-prerelease`
//! (مطابق الگوی `plugin-manifest.schema.json`).
//!
//! نحو بازه‌های پشتیبانی‌شده (فهرست سفید — هر چیز دیگر رد می‌شود):
//! - `*` — هر نسخه
//! - `1.2.3` — دقیقاً
//! - `=1.2.3` — دقیقاً
//! - `^1.2.3` — سازگار با نسخه اصلی (>=1.2.3 و <2.0.0)
//! - `~1.2.3` — سازگار با نسخه فرعی (>=1.2.3 و <1.3.0)
//! - `>=1.2.0` / `>1.2.0` / `<=1.2.0` / `<1.2.0`
//! - ترکیب با کاما: همه شرط‌ها باید برقرار باشند (مثل `>=1.2.0, <2.0.0`)

use serde::{Deserialize, Serialize};
use std::fmt;

/// نسخه معنایی.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// پیش‌انتشار — نسخه با پیش‌انتشار از نسخه نهایی همان سه‌گانه کوچکتر است.
    pub prerelease: Option<String>,
}

impl SemVer {
    /// تجزیه نسخه — خطا با توضیح دقیق.
    pub fn parse(s: &str) -> Result<Self, String> {
        // build metadata (+...) جدا و نادیده گرفته می‌شود (مطابق semver در مقایسه بی‌اثر است)
        let (raw, _) = s
            .split_once('+')
            .map_or((s, ""), |(a, b)| (a, b));
        let (core, prerelease) = match raw.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (raw, None),
        };
        if let Some(p) = &prerelease {
            if p.is_empty() || p.starts_with('.') || p.ends_with('.') || p.contains("..") {
                return Err(format!("invalid prerelease: {p}"));
            }
            if !p.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-') {
                return Err(format!("invalid prerelease: {p}"));
            }
            // شناسه عددی پیش‌انتشار نباید صفر پیش‌رو داشته باشد (مثل 1.0.0-01)
            for ident in p.split('.') {
                if ident.len() > 1 && ident.starts_with('0') && ident.chars().all(|ch| ch.is_ascii_digit()) {
                    return Err(format!("invalid prerelease (leading zero in numeric identifier): {p}"));
                }
            }
        }
        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("expected MAJOR.MINOR.PATCH, got: {s}"));
        }
        let mut nums = [0u64; 3];
        for (i, p) in parts.iter().enumerate() {
            if p.is_empty() || !p.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(format!("invalid numeric part: {s}"));
            }
            nums[i] = p
                .parse()
                .map_err(|_| format!("numeric overflow: {s}"))?;
            // پیش‌زمینه صفر غیرمجاز (مثل 01)
            if p.len() > 1 && p.starts_with('0') {
                return Err(format!("leading zero: {s}"));
            }
        }
        Ok(Self {
            major: nums[0],
            minor: nums[1],
            patch: nums[2],
            prerelease,
        })
    }

    fn triple(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

/// مقایسه شناسه‌های پیش‌انتشار مطابق مشخصه semver:
/// شناسه عددی از شناسه الفبایی کوچکتر است؛ لیست بلندتر (با پیشوند برابر) بزرگتر است.
fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let pa: Vec<&str> = a.split('.').collect();
    let pb: Vec<&str> = b.split('.').collect();
    for (x, y) in pa.iter().zip(pb.iter()) {
        let (xn, yn) = (x.parse::<u64>(), y.parse::<u64>());
        match (xn, yn) {
            (Ok(nx), Ok(ny)) => {
                if nx != ny {
                    return nx.cmp(&ny);
                }
            }
            (Ok(_), Err(_)) => return Ordering::Less, // عددی < الفبایی
            (Err(_), Ok(_)) => return Ordering::Greater,
            (Err(_), Err(_)) => {
                if x != y {
                    return x.cmp(y);
                }
            }
        }
    }
    pa.len().cmp(&pb.len())
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        self.triple().cmp(&other.triple()).then_with(|| {
            match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater, // نسخه نهایی > پیش‌انتشار
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_prerelease(a, b),
            }
        })
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.prerelease {
            Some(p) => write!(f, "{}.{}.{}-{}", self.major, self.minor, self.patch, p),
            None => write!(f, "{}.{}.{}", self.major, self.minor, self.patch),
        }
    }
}

/// شرط یکتای بازه.
#[derive(Debug, Clone, PartialEq)]
enum Constraint {
    Any,
    Exact(SemVer),
    Caret(SemVer),
    Tilde(SemVer),
    Gte(SemVer),
    Gt(SemVer),
    Lte(SemVer),
    Lt(SemVer),
}

impl Constraint {
    fn matches(&self, v: &SemVer) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(e) => v == e,
            Self::Caret(base) => {
                // ^0.y.z → همان 0.y؛ ^0.0.z → همان 0.0.z (نیم‌версوت رسمی semver)
                v >= base
                    && v.major == base.major
                    && (base.major > 0
                        || (v.minor == base.minor && (base.minor > 0 || v.patch == base.patch)))
            }
            Self::Tilde(base) => v >= base && v.major == base.major && v.minor == base.minor,
            Self::Gte(b) => v >= b,
            Self::Gt(b) => v > b,
            Self::Lte(b) => v <= b,
            Self::Lt(b) => v < b,
        }
    }
}

/// بازه سازگاری — هم‌زمانی چند شرط با کاما.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionRange {
    constraints: Vec<Constraint>,
    original: String,
}

impl VersionRange {
    /// تجزیه بازه — ورودی نامعتبر خطا می‌دهد (هرگز ساکت رد نمی‌شود).
    pub fn parse(s: &str) -> Result<Self, String> {
        let text = s.trim();
        if text.is_empty() {
            return Err("empty version range".into());
        }
        let mut constraints = Vec::new();
        for part in text.split(',') {
            let p = part.trim();
            constraints.push(if p == "*" {
                Constraint::Any
            } else if let Some(rest) = p.strip_prefix("^") {
                Constraint::Caret(SemVer::parse(rest)?)
            } else if let Some(rest) = p.strip_prefix("~") {
                Constraint::Tilde(SemVer::parse(rest)?)
            } else if let Some(rest) = p.strip_prefix(">=") {
                Constraint::Gte(SemVer::parse(rest)?)
            } else if let Some(rest) = p.strip_prefix("<=") {
                Constraint::Lte(SemVer::parse(rest)?)
            } else if let Some(rest) = p.strip_prefix('>') {
                Constraint::Gt(SemVer::parse(rest)?)
            } else if let Some(rest) = p.strip_prefix('<') {
                Constraint::Lt(SemVer::parse(rest)?)
            } else if let Some(rest) = p.strip_prefix('=') {
                Constraint::Exact(SemVer::parse(rest)?)
            } else {
                Constraint::Exact(SemVer::parse(p)?)
            });
        }
        Ok(Self { constraints, original: text.to_string() })
    }

    /// آیا نسخه در بازه می‌گنجد؟
    pub fn matches(&self, v: &SemVer) -> bool {
        self.constraints.iter().all(|c| c.matches(v))
    }

    /// متن اصلی بازه.
    pub fn as_str(&self) -> &str {
        &self.original
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> SemVer {
        SemVer::parse(s).unwrap()
    }

    fn range(s: &str) -> VersionRange {
        VersionRange::parse(s).unwrap()
    }

    // ---------- تجزیه نسخه ----------

    #[test]
    fn parse_valid_versions() {
        assert_eq!(v("1.2.3"), SemVer { major: 1, minor: 2, patch: 3, prerelease: None });
        assert_eq!(v("1.2.3-beta.1").prerelease.as_deref(), Some("beta.1"));
        assert_eq!(v("0.0.0").to_string(), "0.0.0");
        // build metadata نادیده گرفته می‌شود
        assert_eq!(v("10.20.30-rc.2+x7").prerelease.as_deref(), Some("rc.2"));
        assert_eq!(v("1.2.3+build"), v("1.2.3"));
    }

    #[test]
    fn parse_rejects_leading_zero_in_prerelease_identifier() {
        assert!(SemVer::parse("1.0.0-01").is_err());
        assert!(SemVer::parse("1.0.0-rc.01").is_err());
        assert!(SemVer::parse("1.0.0-0").is_ok()); // تک‌صفر مجاز
        assert!(SemVer::parse("1.0.0-0alpha").is_ok()); // الفبایی-عددی مجاز
    }

    #[test]
    fn parse_rejects_invalid() {
        for bad in [
            "1.2", "1.2.3.4", "a.b.c", "1.2.x", "", "01.2.3", "1.2.3-", "1.2.3-a..b",
        ] {
            assert!(SemVer::parse(bad).is_err(), "should reject: {bad}");
        }
    }

    #[test]
    fn ordering_with_prerelease() {
        assert!(v("1.2.3-alpha") < v("1.2.3"));
        assert!(v("1.2.2") < v("1.2.3"));
        assert!(v("1.9.9") < v("2.0.0"));
    }

    // ---------- بازه‌ها ----------

    #[test]
    fn exact_and_wildcard() {
        let r = range("1.2.3");
        assert!(r.matches(&v("1.2.3")));
        assert!(!r.matches(&v("1.2.4")));
        assert!(range("*").matches(&v("99.0.0")));
        assert!(range("=2.0.0").matches(&v("2.0.0")));
    }

    #[test]
    fn caret_range() {
        let r = range("^1.2.3");
        assert!(r.matches(&v("1.2.3")));
        assert!(r.matches(&v("1.9.0")));
        assert!(!r.matches(&v("2.0.0")));
        assert!(!r.matches(&v("1.2.2")));
        // نیم‌ورژن صفر
        assert!(range("^0.2.3").matches(&v("0.2.9")));
        assert!(!range("^0.2.3").matches(&v("0.3.0")));
        assert!(range("^0.0.3").matches(&v("0.0.3")));
        assert!(!range("^0.0.3").matches(&v("0.0.4")));
    }

    #[test]
    fn tilde_range() {
        let r = range("~1.2.3");
        assert!(r.matches(&v("1.2.3")));
        assert!(r.matches(&v("1.2.9")));
        assert!(!r.matches(&v("1.3.0")));
    }

    #[test]
    fn comparison_ranges() {
        assert!(range(">=1.2.0").matches(&v("1.2.0")));
        assert!(!range(">1.2.0").matches(&v("1.2.0")));
        assert!(range("<=1.2.0").matches(&v("1.2.0")));
        assert!(!range("<1.2.0").matches(&v("1.2.0")));
        assert!(range("<2.0.0").matches(&v("1.9.9")));
        // پیش‌انتشار کمتر از نهایی است
        assert!(!range(">1.2.3-alpha").matches(&v("1.2.3-alpha")));
    }

    #[test]
    fn conjunction_range() {
        let r = range(">=1.2.0, <2.0.0");
        assert!(r.matches(&v("1.5.0")));
        assert!(!r.matches(&v("2.0.0")));
        assert!(!r.matches(&v("1.1.9")));
    }

    #[test]
    fn invalid_ranges_rejected() {
        for bad in ["", "  ", "abc", ">=1.2", "^1.2.3.4", "1.2.3 >=2.0.0", "<x.0.0"] {
            assert!(VersionRange::parse(bad).is_err(), "should reject: {bad}");
        }
    }

    #[test]
    fn range_preserves_original() {
        assert_eq!(range(">=1.0.0, <2.0.0").as_str(), ">=1.0.0, <2.0.0");
    }
}
