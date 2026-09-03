use std::{collections::HashSet, sync::Arc};

use rustc_hash::FxHashMap;
use time::Date;

use crate::{
    args::Filter,
    db::{Bar, Finance, Market, Metadata},
};

#[derive(Debug)]
pub struct DataFrame {
    pub end: Date,
    pub start: Date,
    /// 索引表
    pub index: Vec<Date>,
    /// 数据列表
    pub list: Vec<Arc<Contract>>,
    /// 板块列表
    pub sector: Arc<HashSet<String>>,
    /// 指数列表
    pub indice: Arc<HashSet<String>>,
}

impl DataFrame {
    /// 按索引表顺序迭代时间索引。
    pub fn index_iter(&self) -> impl Iterator<Item = Index> + '_ {
        self.index.iter().enumerate().map(|(index, datetime)| Index::new(index, *datetime))
    }

    /// 返回指定日期范围内的新数据帧，超出的边界会被裁剪。
    pub fn range(&self, start: Date, end: Date) -> Self {
        let start = start.max(self.start);
        let end = end.min(self.end);
        let index = self
            .index
            .iter()
            .filter(|datetime| **datetime >= start && **datetime <= end)
            .copied()
            .collect();

        Self {
            start,
            end,
            index,
            list: self.list.clone(),
            sector: self.sector.clone(),
            indice: self.indice.clone(),
        }
    }

    /// 返回指定日期范围内并按条件过滤合约的新数据帧。
    ///
    /// 过滤闭包返回 `true` 时保留该合约，返回 `false` 时移除该合约。
    pub fn range_filter<F>(&self, start: Date, end: Date, mut filter: F) -> Self
    where
        F: FnMut(&Arc<Contract>) -> bool,
    {
        let mut frame = self.range(start, end);
        frame.list.retain(|contract| filter(contract));
        frame
    }

    /// 根据参数裁剪日期并过滤合约，板块和指数条件使用并集。
    pub fn filter(&self, args: &Filter) -> Self {
        let has_members_filter = !args.sector.is_empty() || !args.indice.is_empty();

        self.range_filter(args.start, args.end, |contract| {
            let metadata = &contract.metadata;
            if args.filter_bz && metadata.exchange == "北交所" {
                return false;
            }
            if !has_members_filter {
                return true;
            }

            let members = &metadata.members;
            args.sector.iter().any(|sector| members.contains(sector)) || args.indice.iter().any(|indice| members.contains(indice))
        })
    }
}

/// 时间索引
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub index: usize,
    pub datetime: Date,
}

impl Index {
    pub fn new(index: usize, datetime: Date) -> Self {
        Self { index, datetime }
    }
}

/// 合约数据
#[derive(Debug)]
pub struct Contract {
    pub start: Date,
    pub end: Date,
    /// 逐日行情、财务与未来收益数据
    pub bar: Arc<Vec<Bar>>,
    /// 合约元数据
    pub metadata: Metadata,
    /// 时间表
    pub table: FxHashMap<Date, usize>,
}

impl Contract {
    pub fn index(&self, index: &Index) -> Option<usize> {
        self.table.get(&index.datetime).copied()
    }

    pub fn data(&self, i: &Index) -> Option<(&Market, &[f64; 5])> {
        let index = self.index(i)?;
        // 时间表能找到索引必然在范围内
        let bar = unsafe { self.bar.get_unchecked(index) };
        Some((&bar.market, &bar.profit))
    }

    pub fn before(&self, index: &Index, days: usize) -> Option<&Market> {
        let index = self.index(index)?.checked_sub(days)?;
        self.bar.get(index).map(|bar| &bar.market)
    }

    pub fn before_and_profit(&self, index: &Index, days: usize) -> Option<(&Market, &[f64; 5])> {
        let index = self.index(index)?.checked_sub(days)?;
        self.bar.get(index).map(|bar| (&bar.market, &bar.profit))
    }

    pub fn after(&self, index: &Index, days: usize) -> Option<(&Market, &[f64; 5])> {
        let index = self.index(index)?.checked_add(days)?;
        self.bar.get(index).map(|bar| (&bar.market, &bar.profit))
    }

    pub fn data_and_finance(&self, i: &Index) -> Option<(&Market, &[f64; 5], &Finance)> {
        let index = self.index(i)?;
        // 时间表能找到索引必然在范围内
        let bar = unsafe { self.bar.get_unchecked(index) };
        Some((&bar.market, &bar.profit, &bar.finance))
    }
}

#[cfg(test)]
mod tests {
    use time::format_description::well_known::Iso8601;
    use time::{Date, Month};

    use super::*;

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2025, Month::January, day).unwrap()
    }

    fn contract(code: &str, exchange: &str, members: &[&str]) -> Arc<Contract> {
        Arc::new(Contract {
            start: date(1),
            end: date(3),
            metadata: Metadata {
                exchange: exchange.to_string(),
                name: Arc::from(code),
                code: Arc::from(code),
                listing_date: "2020-01-01".to_string(),
                members: members.iter().map(|m| m.to_string()).collect(),
            },
            table: FxHashMap::default(),
            bar: Arc::new(Vec::new()),
        })
    }

    fn frame() -> DataFrame {
        let list = vec![
            contract("000001", "上交所", &["行业一", "沪深指数"]),
            contract("830001", "北交所", &["行业二", "北证指数"]),
        ];

        DataFrame {
            start: date(1),
            end: date(3),
            index: vec![date(1), date(2), date(3)],
            list,
            sector: Arc::new(HashSet::from(["行业一".to_string(), "行业二".to_string()])),
            indice: Arc::new(HashSet::from(["沪深指数".to_string(), "北证指数".to_string()])),
        }
    }

    fn args(sector: HashSet<String>, indice: HashSet<String>, filter_bz: bool, filter_st: bool) -> Filter {
        Filter {
            start: date(2),
            end: date(4),
            filter_bz,
            filter_st,
            sector,
            indice,
        }
    }

    // 测试 before 和 before_and_profit 返回历史数据。
    #[test]
    fn before_returns_historical_data() {
        let mut contract = contract("000001", "上交所", &[]);
        let contract = Arc::get_mut(&mut contract).unwrap();
        contract.bar = Arc::new(
            (1..=3)
                .map(|day| Bar {
                    market: Market {
                        datetime: Date::parse(&format!("2025-01-{day:02}"), &Iso8601::DATE).unwrap(),
                        close: f64::from(day),
                        change_percent: 0.0,
                        open: 0.0,
                        high: 0.0,
                        low: 0.0,
                        volume: 0.0,
                        amount: 0.0,
                        turnover: 0.0,
                        is_st: false,
                    },
                    finance: Finance {
                        total_market: 0.0,
                        dividend_yield: 0.0,
                        ..Finance::default()
                    },
                    profit: [0.1, 0.2, 0.3, 0.4, 0.5],
                })
                .collect(),
        );
        contract.table = contract
            .bar
            .iter()
            .enumerate()
            .map(|(index, bar)| (bar.market.datetime, index))
            .collect::<FxHashMap<_, _>>();

        let index = Index::new(1, date(2));

        let (market, profit) = contract.data(&index).unwrap();
        assert_eq!(market.close, 2.0);
        assert_eq!(profit, &[0.1, 0.2, 0.3, 0.4, 0.5]);

        let prev = contract.before(&index, 1).unwrap();
        assert_eq!(prev.close, 1.0);
        assert!(contract.before(&index, 2).is_none());

        let (prev_market, prev_profit) = contract.before_and_profit(&index, 1).unwrap();
        assert_eq!(prev_market.close, 1.0);
        assert_eq!(prev_profit, &[0.1, 0.2, 0.3, 0.4, 0.5]);

        let next = contract.after(&index, 1).unwrap();
        assert_eq!(next.0.close, 3.0);
        assert_eq!(next.1, &[0.1, 0.2, 0.3, 0.4, 0.5]);
    }

    // 测试 data_and_finance 经 Bar 返回行情与财务。
    #[test]
    fn data_and_finance_returns_bar_market_and_finance() {
        let mut contract = contract("000001", "上交所", &[]);
        let contract = Arc::get_mut(&mut contract).unwrap();
        contract.bar = Arc::new(vec![Bar {
            market: Market {
                datetime: date(2),
                close: 2.0,
                change_percent: 0.0,
                open: 0.0,
                high: 0.0,
                low: 0.0,
                volume: 0.0,
                amount: 0.0,
                turnover: 0.0,
                is_st: false,
            },
            finance: Finance {
                total_market: 2_000.0,
                dividend_yield: 0.0,
                ..Finance::default()
            },
            profit: [0.1, 0.2, 0.3, 0.4, 0.5],
        }]);
        contract.table = contract
            .bar
            .iter()
            .enumerate()
            .map(|(index, bar)| (bar.market.datetime, index))
            .collect::<FxHashMap<_, _>>();

        let index = Index::new(0, date(2));
        let (market, _, finance) = contract.data_and_finance(&index).unwrap();
        assert_eq!(market.close, 2.0);
        assert_eq!(finance.total_market, 2_000.0);
    }

    // 测试板块和指数条件使用并集，任意一项匹配即可保留合约。
    #[test]
    fn from_args_uses_sector_and_indice_union() {
        let frame = frame();
        let filtered = frame.filter(&args(
            HashSet::from(["行业一".to_string()]),
            HashSet::from(["北证指数".to_string()]),
            false,
            false,
        ));

        assert_eq!(filtered.start, date(2));
        assert_eq!(filtered.end, date(3));
        assert_eq!(filtered.index.len(), 2);
        assert_eq!(filtered.list.len(), 2);
        assert!(Arc::ptr_eq(&filtered.sector, &frame.sector));
        assert!(Arc::ptr_eq(&filtered.indice, &frame.indice));
    }

    // 测试 filter_bz 只排除北交所。
    #[test]
    fn from_args_filters_beijing_exchange_only() {
        let frame = frame();
        let filtered = frame.filter(&args(HashSet::new(), HashSet::new(), true, false));

        assert_eq!(filtered.list.len(), 1);
        assert_eq!(filtered.list[0].metadata.exchange, "上交所");
        assert_eq!(filtered.sector.len(), 2);
        assert_eq!(filtered.indice.len(), 2);
    }

    // 测试 filter_st 不在合约层过滤，具体 ST 日期由因子循环根据行情判断。
    #[test]
    fn from_args_defers_st_filtering_to_market_data() {
        let mut frame = frame();
        frame.list.push(contract("ST0001", "上交所", &[]));

        let filtered = frame.filter(&args(HashSet::new(), HashSet::new(), false, true));

        assert_eq!(filtered.list.len(), 3);
        assert!(filtered.list.iter().any(|contract| contract.metadata.name.contains("ST")));
    }

    // 测试板块和指数均不匹配时过滤全部合约，但不修改原始列表信息。
    #[test]
    fn from_args_keeps_metadata_lists_when_no_contract_matches() {
        let frame = frame();
        let filtered = frame.filter(&args(
            HashSet::from(["不存在的板块".to_string()]),
            HashSet::from(["不存在的指数".to_string()]),
            false,
            false,
        ));

        assert!(filtered.list.is_empty());
        assert!(Arc::ptr_eq(&filtered.sector, &frame.sector));
        assert!(Arc::ptr_eq(&filtered.indice, &frame.indice));
    }
}
