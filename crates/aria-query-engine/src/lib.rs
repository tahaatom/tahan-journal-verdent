//! aria-query-engine — موتور پرس‌وجوی کرنل آریا
//!
//! شامل: فیلترهای ساده و مرکب (AND/OR)، فیلتر فیلدهای سفارشی، تجمیع‌ها
//! (نرخ برد، میانگین R، افت سرمایه، منحنی سرمایه)، عملکرد گروهی،
//! پروجکشن داشبورد با باطل‌سازی رویدادمحور، و بودجه پرس‌وجو.
//!
//! پلاگین‌ها هرگز به SQL مستقیم دسترسی ندارند؛ تنها از طریق این موتور.

pub mod error;
pub mod filter;
pub mod projection;
pub mod service;
pub mod stats;

pub use error::QueryError;
pub use filter::{
    CustomFieldFilter, CustomFieldOp, CustomValue, FilterNode, MAX_FILTER_DEPTH, TradeFilter,
    TradeResult,
};
pub use projection::DailySummaryRow;
pub use service::{
    DashboardReport, PagedTrades, QueryService, StatFieldInfo, TradeListRow, MAX_PAGE_SIZE,
};
pub use stats::{CoreStats, Dimension, EquityPoint, GroupStat, HeatCell};

/// نسخه قرارداد پرس‌وجو.
pub const QUERY_CONTRACT_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn contract_version_is_stable() {
        assert_eq!(super::QUERY_CONTRACT_VERSION, 1);
        assert_eq!(super::MAX_PAGE_SIZE, 200);
    }
}
