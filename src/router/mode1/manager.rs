//! Mode1列表数据管理
use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio::{
    sync::{RwLock, broadcast::Receiver},
    task::JoinSet,
};

use crate::{DF, args::Filter, cache::Cache, db::{Finance, Market, Metadata}};

#[derive(Debug, Serialize, Deserialize)]
pub struct ListItem {
    pub args: Arc<RawValue>,
    pub data: Arc<RawValue>,
}

type Mode1Fn = Arc<dyn Fn(&Filter) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) + Send + Sync + 'static>;

pub struct Mode1Manager {
    inner: RwLock<Vec<Mode1Fn>>,
    pub cache: Cache,
    pub details: Cache,
}

impl Mode1Manager {
    pub fn new(base: &Path) -> Self {
        Self {
            inner: RwLock::new(Vec::new()),
            cache: Cache::sub(base, "mode1").expect("创建 mode1 缓存目录失败"),
            details: Cache::sub(base, "mode1-details").expect("创建 mode1-details 缓存目录失败"),
        }
    }
}

impl Mode1Manager {
    pub async fn register(&self, func: Mode1Fn) {
        self.inner.write().await.push(func);
    }

    pub async fn execute(&self, filter: &Filter) -> Vec<ListItem> {
        let funcs: Vec<Mode1Fn> = self.inner.read().await.iter().map(Arc::clone).collect();
        let filter = Arc::new(filter.clone());
        let mut tasks = JoinSet::new();

        for func in funcs {
            let filter = Arc::clone(&filter);
            tasks.spawn(async move {
                let (args, mut recv) = func(&filter);
                let data = recv.recv().await.unwrap();
                ListItem { args, data }
            });
        }
        tasks.join_all().await
    }
}

// ─── Mode1Data（分位数据容器）───────────────────────────────────────────────

use salvo_oapi::ToSchema;
use time::Date;

use crate::math::{avg_array, avg_iter};
use crate::model::Profit;

/// 分位数据
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Mode1Data {
    pub id: String,
    pub name: String,
    pub info: String,
    pub label: String,
    pub count: usize,
    pub factor: Vec<Vec<f64>>,
    pub turnover_rate: Vec<Vec<f64>>,
    pub profit1: Vec<Profit>,
    pub profit2: Vec<Profit>,
    pub profit3: Vec<Profit>,
    pub profit4: Vec<Profit>,
    #[serde(serialize_with = "crate::toolbox::serde::date_format::serialize_datetime")]
    pub datetime: Vec<Date>,
}

/// 每个股票当期数据
pub struct Mode1Temp<'a> {
    pub factor: f64,
    pub profit: &'a [f64; 5],
}

/// 单日分位明细中的一行（一只股票）。
#[derive(Debug, Clone, Serialize)]
pub struct DetailRow<'a> {
    /// 证券代码（带后缀）
    pub code: &'a str,
    /// 证券名称
    pub name: &'a str,
    /// 交易所
    pub exchange: &'a str,
    /// 所属行业/指数分类（升序）
    pub tags: Vec<&'a str>,
    /// 因子值
    pub factor: f64,
    /// 前向收益 `[p1, p2, p3, p4, 换手率]`
    pub profit: &'a [f64; 5],
    /// 当日行情字段（datetime/OHLCV/turnover/is_st 等）
    #[serde(flatten)]
    pub market: &'a Market,
    /// 当日财务字段（总市值/股本/股息率/同比等）
    pub finance: &'a Finance,
}

impl<'a> DetailRow<'a> {
    pub fn new(metadata: &'a Metadata, market: &'a Market, finance: &'a Finance, factor: f64, profit: &'a [f64; 5]) -> Self {
        let mut tags: Vec<&str> = metadata.members.iter().map(String::as_str).collect();
        tags.sort_unstable();
        Self {
            code: metadata.code.as_ref(),
            name: metadata.name.as_ref(),
            exchange: metadata.exchange.as_str(),
            tags,
            factor,
            profit,
            market,
            finance,
        }
    }
}

/// 目标日全市场分位明细响应。
#[derive(Debug, Serialize)]
pub struct QuantileDay<'a> {
    /// 查询的目标日期
    #[serde(with = "crate::toolbox::serde::date_format")]
    pub date: Date,
    /// 分位数量
    pub count: usize,
    /// 分位 `0..count`；每分位为按因子升序切分后的当日股票行。
    /// 股票数不足分位数时全分位共享当日集合（与 `Mode1Data::push` 一致）。
    pub quantiles: Vec<Vec<DetailRow<'a>>>,
}

/// 把当日全市场因子行按 `count` 分位切分并序列化。
///
/// 排序方向与整数边界 `index*len/count` 完全复用 `Mode1Data::push` 的口径。
pub fn day_value<'a>(date: Date, count: usize, rows: Vec<DetailRow<'a>>) -> Box<RawValue> {
    let quantiles = split_quantiles(count, rows);
    let value = QuantileDay { date, count, quantiles };
    let s = serde_json::to_string(&value).unwrap();
    RawValue::from_string(s).unwrap()
}

/// 按 `Mode1Data::push` 同款边界把行切为 `count` 组（内部先按因子升序排序）。
fn split_quantiles<'a>(count: usize, rows: Vec<DetailRow<'a>>) -> Vec<Vec<DetailRow<'a>>> {
    if count == 0 {
        return Vec::new();
    }
    if rows.is_empty() {
        return vec![Vec::new(); count];
    }
    let mut rows = rows;
    rows.sort_unstable_by(|left, right| left.factor.total_cmp(&right.factor));
    let len = rows.len();
    if len < count {
        // 与主接口一致：股票数少于分位数时全分位共享当日集合。
        return (0..count).map(|_| rows.clone()).collect();
    }
    let bounds: Vec<usize> = (0..=count).map(|index| index * len / count).collect();
    let mut groups: Vec<Vec<DetailRow<'a>>> = vec![Vec::new(); count];
    for (index, row) in rows.into_iter().enumerate() {
        let group = bounds.partition_point(|&bound| bound <= index) - 1;
        groups[group].push(row);
    }
    groups
}

/// 解析单日明细的目标日期；缺省时取筛选区间内的末交易日。
pub fn resolve_detail_date(date: Option<Date>, filter: &Filter) -> Date {
    date.unwrap_or_else(|| {
        let index = &DF.index;
        let pos = index.partition_point(|day| *day <= filter.end);
        if pos == 0 {
            DF.start
        } else {
            index[pos - 1]
        }
    })
}

/// 构造单日明细的过滤条件：终点 = 目标日，起点按 `warmup` 个交易日回推，股票池条件不变。
pub fn detail_filter(filter: &Filter, date: Date, warmup: usize) -> Filter {
    let mut result = filter.clone();
    result.end = date;
    result.start = DF.warmup_start(date, warmup);
    result
}

impl Mode1Data {
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<String>, info: impl Into<String>, label: impl Into<String>, count: usize) -> Self {
        assert!(count > 0, "分位数量必须大于 0");
        Self {
            id: id.into().to_string(),
            name: name.into(),
            info: info.into(),
            label: label.into(),
            count,
            factor: vec![Vec::new(); count],
            turnover_rate: vec![Vec::new(); count],
            profit1: vec![Profit::new(); count],
            profit2: vec![Profit::new(); count],
            profit3: vec![Profit::new(); count],
            profit4: vec![Profit::new(); count],
            datetime: Vec::new(),
        }
    }

    pub fn push(&mut self, datetime: Date, items: &mut [Mode1Temp<'_>]) {
        if items.is_empty() {
            return;
        }
        items.sort_unstable_by(|left, right| left.factor.total_cmp(&right.factor));
        let len = items.len();
        for index in 0..self.count {
            let (start, end) = if len < self.count {
                (0, len)
            } else {
                (index * len / self.count, (index + 1) * len / self.count)
            };
            let group = unsafe { items.get_unchecked(start..end) };
            let factor = avg_iter(group.iter().map(|item| item.factor));
            let [p1, p2, p3, p4, tr] = avg_array(group.iter().map(|item| item.profit));
            self.factor[index].push(factor);
            self.turnover_rate[index].push(tr);
            self.profit1[index].push(p1);
            self.profit2[index].push(p2);
            self.profit3[index].push(p3);
            self.profit4[index].push(p4);
        }
        self.datetime.push(datetime);
    }

    pub fn raw_value(&mut self) -> Box<RawValue> {
        self.update_annualized_profit();
        let s = serde_json::to_string(self).unwrap();
        RawValue::from_string(s).unwrap()
    }

    fn update_annualized_profit(&mut self) {
        let Some(days) = self.annualized_days() else { return };
        for profits in [&mut self.profit1, &mut self.profit2, &mut self.profit3, &mut self.profit4] {
            for profit in profits {
                profit.update_annualized_profit(days);
            }
        }
    }

    fn annualized_days(&self) -> Option<f64> {
        let start = *self.datetime.first()?;
        let end = *self.datetime.last()?;
        let days = (end - start).whole_days();
        if days <= 0 { None } else { Some(days as f64) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profit(factor: f64) -> [f64; 5] {
        [factor / 100.0, factor / 10.0, -factor / 100.0, factor, factor * 10.0]
    }

    fn items<'a>(factors: &[f64], profits: &'a [[f64; 5]]) -> Vec<Mode1Temp<'a>> {
        factors
            .iter()
            .copied()
            .zip(profits)
            .map(|(factor, profit)| Mode1Temp { factor, profit })
            .collect()
    }

    #[test]
    fn raw_value_updates_annualized_profit_by_datetime_span() {
        let mut data = Mode1Data::new("test", "测试策略", "", "测试标签", 2);
        data.datetime.push(Date::from_calendar_date(2025, time::Month::January, 1).unwrap());
        data.datetime.push(Date::from_calendar_date(2025, time::Month::January, 31).unwrap());
        data.profit1[0].push(0.1);
        data.profit2[0].push(0.2);
        let _ = data.raw_value();
        let expected1 = 0.1 / 30.0 * 365.0;
        let expected2 = 0.2 / 30.0 * 365.0;
        assert!((data.profit1[0].annualized_profit - expected1).abs() < 1e-12);
        assert!((data.profit2[0].annualized_profit - expected2).abs() < 1e-12);
        assert_eq!(data.profit3[0].annualized_profit, 0.0);
        assert_eq!(data.profit4[0].annualized_profit, 0.0);
    }

    #[test]
    fn raw_value_ignores_annualized_profit_without_datetime() {
        let mut data = Mode1Data::new("test", "测试策略", "空时间", "测试标签", 1);
        data.profit1[0].push(0.1);
        let _ = data.raw_value();
        assert_eq!(data.profit1[0].annualized_profit, 0.0);
    }

    #[test]
    fn raw_value_serializes_datetime_as_string_array() {
        let mut data = Mode1Data::new("test", "测试策略", "时间序列", "测试标签", 1);
        data.datetime.push(Date::from_calendar_date(2025, time::Month::January, 1).unwrap());
        data.datetime.push(Date::from_calendar_date(2025, time::Month::January, 2).unwrap());

        let raw = data.raw_value();
        let value: serde_json::Value = serde_json::from_str(raw.get()).unwrap();

        assert_eq!(value["datetime"], serde_json::json!(["2025-01-01", "2025-01-02"]));
    }

    #[test]
    fn mode1data_new_initializes_groups() {
        let data = Mode1Data::new("test", "价值策略", "按因子从低到高分组", "基础科目及衍生类因子", 3);
        assert_eq!(data.id, "test");
        assert_eq!(data.name, "价值策略");
        assert_eq!(data.info, "按因子从低到高分组");
        assert_eq!(data.label, "基础科目及衍生类因子");
        assert_eq!(data.count, 3);
        assert_eq!(data.factor.len(), 3);
        assert_eq!(data.turnover_rate.len(), 3);
        assert_eq!(data.profit1.len(), 3);
        assert_eq!(data.profit2.len(), 3);
        assert_eq!(data.profit3.len(), 3);
        assert_eq!(data.profit4.len(), 3);
        assert!(data.datetime.is_empty());
        assert!(data.factor.iter().all(Vec::is_empty));
        assert!(data.turnover_rate.iter().all(Vec::is_empty));
    }

    #[test]
    fn mode1data_push_sorts_and_splits_items() {
        let mut data = Mode1Data::new("test", "测试策略", "两分位", "测试标签", 2);
        let factors = [4.0, 1.0, 3.0, 2.0];
        let profits = factors.map(profit);
        let mut items = items(&factors, &profits);
        data.push(Date::from_calendar_date(2025, time::Month::January, 1).unwrap(), &mut items);
        assert_eq!(items.iter().map(|item| item.factor).collect::<Vec<_>>(), [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(data.datetime[0].to_string(), "2025-01-01");
        assert_eq!(data.factor, [vec![1.5], vec![3.5]]);
        assert_eq!(data.turnover_rate, [vec![15.0], vec![35.0]]);
        assert!((data.profit1[0].source[0] - 0.015).abs() < 1e-12);
        assert!((data.profit1[1].source[0] - 0.035).abs() < 1e-12);
    }

    #[test]
    fn mode1data_push_uses_integer_boundaries() {
        let mut data = Mode1Data::new("test", "测试策略", "三分位", "测试标签", 3);
        let factors = [5.0, 1.0, 4.0, 2.0, 3.0];
        let profits = factors.map(profit);
        let mut items = items(&factors, &profits);
        data.push(Date::from_calendar_date(2025, time::Month::January, 2).unwrap(), &mut items);
        assert_eq!(data.factor, [vec![1.0], vec![2.5], vec![4.5]]);
    }

    #[test]
    fn mode1data_shares_items_when_count_is_insufficient() {
        let mut data = Mode1Data::new("test", "测试策略", "四分位", "测试标签", 4);
        let factors = [3.0, 1.0];
        let profits = factors.map(profit);
        let mut items = items(&factors, &profits);
        data.push(Date::from_calendar_date(2025, time::Month::January, 3).unwrap(), &mut items);
        assert_eq!(data.factor, [vec![2.0], vec![2.0], vec![2.0], vec![2.0]]);
        assert_eq!(data.turnover_rate, [vec![20.0], vec![20.0], vec![20.0], vec![20.0]]);
        assert!(data.profit1.iter().all(|p| (p.source[0] - 0.02).abs() < 1e-12));
        assert_eq!(data.datetime[0].to_string(), "2025-01-03");
    }

    #[test]
    fn mode1data_ignores_empty_items() {
        let mut data = Mode1Data::new("test", "测试策略", "空数据", "测试标签", 3);
        let mut items: Vec<Mode1Temp<'_>> = Vec::new();
        data.push(Date::from_calendar_date(2025, time::Month::January, 4).unwrap(), &mut items);
        assert!(data.datetime.is_empty());
        assert!(data.factor.iter().all(Vec::is_empty));
        assert!(data.profit1.iter().all(|p| p.source.is_empty()));
    }

    // ─── 单日分位明细切分 ───────────────────────────────────────────────
    use std::collections::HashSet as FxSet;

    fn leaked_row(factor: f64) -> (&'static Metadata, &'static Market, &'static Finance, &'static [f64; 5]) {
        let metadata = Box::leak(Box::new(Metadata {
            code: Arc::from("000001.SZ"),
            name: Arc::from("测试股票"),
            exchange: "上交所".to_string(),
            listing_date: "2000-01-01".to_string(),
            members: FxSet::from(["行业一".to_string()]),
        }));
        let market = Box::leak(Box::new(Market {
            datetime: Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
            change_percent: 0.0,
            open: factor,
            close: factor,
            high: factor,
            low: factor,
            volume: 0.0,
            amount: 0.0,
            turnover: 0.0,
            is_st: false,
        }));
        let profit = Box::leak(Box::new([0.0; 5]));
        let finance = Box::leak(Box::new(Finance {
            total_market: 0.0,
            dividend_yield: 0.0,
            ..Finance::default()
        }));
        (metadata, market, finance, profit)
    }

    fn detail_rows(factors: &[f64]) -> Vec<DetailRow<'static>> {
        factors
            .iter()
            .copied()
            .map(|factor| {
                let (metadata, market, finance, profit) = leaked_row(factor);
                DetailRow::new(metadata, market, finance, factor, profit)
            })
            .collect()
    }

    #[test]
    fn split_quantiles_groups_by_ascending_factor() {
        let rows = detail_rows(&[4.0, 1.0, 3.0, 2.0]);
        let groups = split_quantiles(2, rows);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].iter().map(|row| row.factor).collect::<Vec<_>>(), [1.0, 2.0]);
        assert_eq!(groups[1].iter().map(|row| row.factor).collect::<Vec<_>>(), [3.0, 4.0]);
    }

    #[test]
    fn split_quantiles_uses_integer_boundaries() {
        let rows = detail_rows(&[5.0, 1.0, 4.0, 2.0, 3.0]);
        let groups = split_quantiles(3, rows);
        assert_eq!(groups[0].iter().map(|row| row.factor).collect::<Vec<_>>(), [1.0]);
        assert_eq!(groups[1].iter().map(|row| row.factor).collect::<Vec<_>>(), [2.0, 3.0]);
        assert_eq!(groups[2].iter().map(|row| row.factor).collect::<Vec<_>>(), [4.0, 5.0]);
    }

    #[test]
    fn split_quantiles_shares_rows_when_count_exceeds_len() {
        let rows = detail_rows(&[3.0, 1.0]);
        let groups = split_quantiles(4, rows);
        assert_eq!(groups.len(), 4);
        for group in &groups {
            assert_eq!(group.iter().map(|row| row.factor).collect::<Vec<_>>(), [1.0, 3.0]);
        }
    }

    #[test]
    fn split_quantiles_empty_rows_yield_empty_groups() {
        let groups = split_quantiles(3, Vec::new());
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().all(Vec::is_empty));
    }

    #[test]
    fn detail_row_carries_metadata_and_market() {
        let (metadata, market, finance, profit) = leaked_row(9.5);
        let row = DetailRow::new(metadata, market, finance, 2.0, profit);
        assert_eq!(row.code, "000001.SZ");
        assert_eq!(row.exchange, "上交所");
        assert_eq!(row.tags, ["行业一"]);
        assert_eq!(row.factor, 2.0);
        assert_eq!(row.market.close, 9.5);
    }
}
