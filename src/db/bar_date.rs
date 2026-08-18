use serde::{Deserialize, Serialize};
use time::Date;

/// 行情数据。
///
/// 新数据源（`data/market/<code>.json`）的字段与旧结构存在映射差异：
/// - 数据源 `change_pct` → 本结构 `change_percent`（反序列化别名）
/// - 数据源 `amount`（成交额）→ 本结构 `amount`
/// - 数据源 `turnover`（换手率）→ 本结构 `turnover`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    /// 日期时间（例如：2025-03-15）
    #[serde(with = "crate::toolbox::serde::date_format")]
    pub datetime: Date,
    /// 涨幅（百分比）
    #[serde(alias = "change_pct")]
    pub change_percent: f64,
    /// 开盘价
    pub open: f64,
    /// 收盘价
    pub close: f64,
    /// 最高价
    pub high: f64,
    /// 最低价
    pub low: f64,
    /// 成交量
    pub volume: f64,
    /// 成交额
    pub amount: f64,
    /// 换手率（百分比）
    pub turnover: f64,
    /// 是否为ST
    pub is_st: bool,
}

impl Market {
    /// 根据 ST 过滤开关判断是否保留当日行情，返回 `true` 时保留。
    #[inline]
    pub fn filter_st(&self, filter_st: bool) -> bool {
        !filter_st || !self.is_st
    }
}

/// 财务数据（合并进行情 JSON 行）。
///
/// 新数据源中财务字段与行情合并于同一行，仅 `total_market`（总市值）被因子使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finance {
    /// 总市值（单位：元）
    pub total_market: f64,
}

/// 单交易日数据：行情 + 财务。
#[derive(Debug, Clone)]
pub struct Bar {
    /// 行情数据
    pub market: Market,
    /// 财务数据
    pub finance: Finance,
}

impl Bar {
    /// 根据 ST 过滤开关判断是否保留当日行情，返回 `true` 时保留。
    #[inline]
    pub fn filter_st(&self, filter_st: bool) -> bool {
        self.market.filter_st(filter_st)
    }

    /// 返回当日总市值（单位：元）。
    #[inline]
    pub fn total_market(&self) -> f64 {
        self.finance.total_market
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn market(datetime: &str, is_st: bool) -> Market {
        Market {
            datetime: Date::parse(datetime, &time::format_description::well_known::Iso8601::DATE).unwrap(),
            change_percent: 0.01,
            open: 10.0,
            close: 11.0,
            high: 12.0,
            low: 9.0,
            volume: 100.0,
            amount: 1_000.0,
            turnover: 0.02,
            is_st,
        }
    }

    // 测试未启用过滤时保留全部行情，启用后仅过滤当日 ST 行情。
    #[test]
    fn filter_st_uses_daily_market_status() {
        let normal = market("2025-01-01", false);
        let st = market("2025-01-01", true);

        assert!(normal.filter_st(false));
        assert!(st.filter_st(false));
        assert!(normal.filter_st(true));
        assert!(!st.filter_st(true));
    }

    // 测试新数据源字段名（change_pct）可以反序列化为 change_percent。
    #[test]
    fn deserializes_new_data_source_field_names() {
        let row: Market = serde_json::from_str(
            r#"{"datetime":"2025-01-01","change_pct":1.5,"open":10.0,"close":11.0,"high":12.0,"low":9.0,"volume":100.0,"amount":1000.0,"turnover":0.02,"is_st":false}"#,
        )
        .unwrap();

        assert_eq!(row.datetime.to_string(), "2025-01-01");
        assert_eq!(row.change_percent, 1.5);
        assert_eq!(row.amount, 1000.0);
        assert_eq!(row.turnover, 0.02);
    }

    // 测试 Bar 聚合行情与财务，filter_st 与 total_market 便捷方法正确。
    #[test]
    fn bar_aggregates_market_and_finance() {
        let bar = Bar {
            market: market("2025-01-01", false),
            finance: Finance { total_market: 1_000.0 },
        };

        assert!(bar.filter_st(true));
        assert_eq!(bar.total_market(), 1_000.0);
        assert_eq!(bar.market.close, 11.0);
    }
}
