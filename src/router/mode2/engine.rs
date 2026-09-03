//! 模式二：因子选股引擎（逐日横截面选股与区间组合回测）。

use rustc_hash::FxHashMap;
use salvo_oapi::ToSchema;
use serde::Serialize;
use time::Date;

use crate::{
    db::{Bar, DataFrame, Metadata},
    router::mode2::{Req, operator::Operator},
};

/// 单日选股名单条目。
#[derive(Debug, Serialize, ToSchema)]
pub struct StockItem {
    /// 裸代码
    pub code: String,
    /// 名称
    pub name: String,
    /// 因子值
    pub factor: f64,
    /// 当日涨跌幅（百分比）
    pub change_percent: f64,
    /// 是否为 ST
    pub is_st: bool,
    /// 交易所
    pub exchange: String,
    /// 行业/指数标签
    pub tags: Vec<String>,
    /// 当日开盘价
    pub open: f64,
    /// 当日最高价
    pub high: f64,
    /// 当日最低价
    pub low: f64,
    /// 当日收盘价
    pub close: f64,
    /// 当日成交量
    pub volume: f64,
    /// 当日成交额
    pub amount: f64,
    /// 当日换手率（百分比）
    pub turnover: f64,
}

/// 区间回测统计指标。
#[derive(Debug, Serialize, ToSchema)]
pub struct Mode2Stats {
    /// 总收益（末净值 - 1）
    pub total_profit: f64,
    /// 年化收益（总收益 / 天数 × 365）
    pub annualized: f64,
    /// 最大回撤（净值相对历史峰值的最大跌幅，负值；单调上涨为 0）
    pub max_drawdown: f64,
    /// 胜率（组合日收益跑赢基准的天数占比）
    pub win_rate: f64,
}

/// 区间回测结果。
#[derive(Debug, Serialize, ToSchema)]
pub struct Mode2History {
    /// 交易日时间轴
    #[serde(serialize_with = "crate::toolbox::serde::date_format::serialize_datetime")]
    pub datetime: Vec<Date>,
    /// 组合净值（每日调仓等权，首点 1.0）
    pub portfolio: Vec<f64>,
    /// 基准净值（同股票池等权，首点 1.0）
    pub benchmark: Vec<f64>,
    /// 每日调仓换手率（首日 1.0，空名单日 0.0）
    pub turnover: Vec<f64>,
    /// 每日入选数量
    pub count: Vec<usize>,
    /// 统计指标
    pub stats: Mode2Stats,
}

/// 当日收集池：符合股票池（frame 已过滤）与 ST 过滤的全部行情引用。
///
/// 返回 `bar` 引用集合与「`bar` 地址 → 元数据」映射；`&Bar` 源自
/// `Arc<Vec<Bar>>` 地址稳定，raw pointer 仅作 map key 不 deref。
fn collect_bars<'a>(
    frame: &'a DataFrame,
    args: &Req,
    date: Date,
) -> (Vec<&'a Bar>, FxHashMap<*const Bar, &'a Metadata>) {
    let mut bars = Vec::new();
    let mut meta = FxHashMap::default();
    for contract in &frame.list {
        let Some(pos) = contract.table.get(&date).copied() else { continue };
        let bar = &contract.bar[pos];
        if !bar.market.filter_st(args.base.filter_st) {
            continue;
        }
        meta.insert(std::ptr::from_ref(bar), &contract.metadata);
        bars.push(bar);
    }
    (bars, meta)
}

/// 构造选股算子链（stages 顺序执行）。
fn operators_of(args: &Req) -> Vec<Operator> {
    args.stages
        .iter()
        .map(|stage| Operator {
            field: stage.field,
            filter: stage.filter,
            select: Some(stage.select),
            direction: stage.direction,
        })
        .collect()
}

/// 单日选股：按算子链（排序 → 过滤 → 截取）顺序执行，返回名单。
pub fn select_at(frame: &DataFrame, args: &Req, date: Date) -> Vec<StockItem> {
    let (mut bars, meta) = collect_bars(frame, args, date);
    let operators = operators_of(args);
    let last_operator = operators.last().expect("算子链至少 1 段");
    let mut selected: &mut [&Bar] = &mut bars;
    for operator in &operators {
        selected = operator.run(selected);
    }

    selected
        .iter()
        .map(|bar| {
            let metadata = meta
                .get(&std::ptr::from_ref(*bar))
                .expect("选中 bar 必有元数据");
            let market = &bar.market;
            let mut tags = metadata.members.iter().cloned().collect::<Vec<_>>();
            tags.sort();
            StockItem {
                code: metadata.code.to_string(),
                name: metadata.name.to_string(),
                factor: last_operator.get_field(bar),
                change_percent: market.change_percent,
                is_st: market.is_st,
                exchange: metadata.exchange.clone(),
                tags,
                open: market.open,
                high: market.high,
                low: market.low,
                close: market.close,
                volume: market.volume,
                amount: market.amount,
                turnover: market.turnover,
            }
        })
        .collect()
}

/// 区间回测：逐日选股，等权组合净值、同池基准、调仓换手率与统计。
///
/// 空名单日：组合收益 0（净值持平）、换手率 0、count 0、日期保留。
pub fn history(frame: &DataFrame, args: &Req) -> Mode2History {
    let mode = (args.profit_mode - 1) as usize;
    let mut datetime = Vec::with_capacity(frame.index.len() + 1);
    let mut portfolio = Vec::with_capacity(frame.index.len() + 1);
    let mut benchmark = Vec::with_capacity(frame.index.len() + 1);
    let mut turnover = Vec::with_capacity(frame.index.len() + 1);
    let mut count = Vec::with_capacity(frame.index.len() + 1);
    let mut nav = 1.0_f64;
    let mut bench_nav = 1.0_f64;
    let mut prev: Vec<String> = Vec::new();
    let mut wins = 0usize;

    // 首点基线：期初净值 1.0（与首个交易日对齐）。
    if let Some(first) = frame.index.first() {
        datetime.push(*first);
        portfolio.push(1.0);
        benchmark.push(1.0);
        turnover.push(0.0);
        count.push(0);
    }

    for date in &frame.index {
        let (mut bars, meta) = collect_bars(frame, args, *date);

        // 基准：与选股同股票池（含 ST 过滤）的等权收益。
        let b_t = if bars.is_empty() {
            0.0
        } else {
            bars.iter().map(|bar| bar.profit[mode]).sum::<f64>() / bars.len() as f64
        };

        let operators = operators_of(args);
        let mut selected: &mut [&Bar] = &mut bars;
        for operator in &operators {
            selected = operator.run(selected);
        }
        let mut codes = Vec::with_capacity(selected.len());
        let mut sum = 0.0_f64;
        for bar in selected {
            sum += bar.profit[mode];
            codes.push(
                meta.get(&std::ptr::from_ref(bar))
                    .expect("选中 bar 必有元数据")
                    .code
                    .to_string(),
            );
        }
        let r_t = if codes.is_empty() { 0.0 } else { sum / codes.len() as f64 };

        // 调仓换手率：首日 1.0；空名单日 0.0；否则按与前日名单的代码交集。
        let tr = if codes.is_empty() {
            0.0
        } else if prev.is_empty() {
            1.0
        } else {
            let keep = codes.iter().filter(|code| prev.contains(code)).count();
            1.0 - keep as f64 / codes.len() as f64
        };

        nav *= 1.0 + r_t;
        bench_nav *= 1.0 + b_t;
        if r_t > b_t {
            wins += 1;
        }
        datetime.push(*date);
        portfolio.push(nav);
        benchmark.push(bench_nav);
        turnover.push(tr);
        count.push(codes.len());
        prev = codes;
    }

    let days = frame.index.len() as f64;
    let stats = if days > 0.0 {
        let total_profit = nav - 1.0;
        Mode2Stats {
            total_profit,
            annualized: total_profit / days * 365.0,
            max_drawdown: drawdown(&portfolio),
            win_rate: wins as f64 / days,
        }
    } else {
        Mode2Stats {
            total_profit: 0.0,
            annualized: 0.0,
            max_drawdown: 0.0,
            win_rate: 0.0,
        }
    };

    Mode2History {
        datetime,
        portfolio,
        benchmark,
        turnover,
        count,
        stats,
    }
}

/// 最大回撤：净值相对历史峰值的最大跌幅（≤ 0）。
fn drawdown(nav: &[f64]) -> f64 {
    let mut peak = f64::NEG_INFINITY;
    let mut min_dd = 0.0_f64;
    for &value in nav {
        peak = peak.max(value);
        min_dd = min_dd.min(value / peak - 1.0);
    }
    min_dd
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use rustc_hash::FxHashMap;
    use time::Month;

    use crate::{
        args::Filter as PoolFilter,
        db::{Bar, Contract, Finance, Market, Metadata},
        router::mode2::{
            Req, Stage,
            operator::{Direction, Field, Filter as OpFilter},
        },
    };

    use super::*;

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2025, Month::January, day).unwrap()
    }

    fn market(close: f64, change_percent: f64, is_st: bool) -> Market {
        Market {
            datetime: date(1),
            change_percent,
            open: close,
            close,
            high: close,
            low: close,
            volume: 1_000.0,
            amount: 10_000.0,
            turnover: 1.0,
            is_st,
        }
    }

    fn bar(close: f64, change_percent: f64, is_st: bool, profit: [f64; 5]) -> Bar {
        Bar {
            market: market(close, change_percent, is_st),
            finance: Finance {
                total_market: close,
                dividend_yield: close / 100.0,
                ..Finance::default()
            },
            profit,
        }
    }

    fn metadata(code: &str, name: &str, members: &[&str]) -> Metadata {
        Metadata {
            exchange: "上交所".into(),
            name: Arc::from(name),
            code: Arc::from(code),
            listing_date: "2020-01-01".into(),
            members: members.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn contract(code: &str, name: &str, days: &[Date], bars: Vec<Bar>, members: &[&str]) -> Arc<Contract> {
        let table = days
            .iter()
            .enumerate()
            .map(|(index, &day)| (day, index))
            .collect::<FxHashMap<_, _>>();
        Arc::new(Contract {
            start: days[0],
            end: days[days.len() - 1],
            bar: Arc::new(bars),
            metadata: metadata(code, name, members),
            table,
        })
    }

    /// 三只股票 × 三日 fixture：
    /// - A(000001 平安) close 10/11/12，profit[0] = 0.01/0.04/0
    /// - B(000002 万科) close 20/18/19，profit[0] = 0.02/0.05/0
    /// - C(000003 招商) close 30/32/31，day2 为 ST，profit[0] = 0.03/0.06/0
    ///   profit 其余分量与 day3（尾部）全 0。
    fn frame() -> DataFrame {
        let days = [date(1), date(2), date(3)];
        let list = vec![
            contract(
                "000001",
                "平安",
                &days,
                vec![
                    bar(10.0, 0.0, false, [0.01, 0.0, 0.0, 0.0, 0.0]),
                    bar(11.0, 1.0, false, [0.04, 0.0, 0.0, 0.0, 0.0]),
                    bar(12.0, 2.0, false, [0.0; 5]),
                ],
                &["银行"],
            ),
            contract(
                "000002",
                "万科",
                &days,
                vec![
                    bar(20.0, 0.0, false, [0.02, 0.0, 0.0, 0.0, 0.0]),
                    bar(18.0, 2.0, false, [0.05, 0.0, 0.0, 0.0, 0.0]),
                    bar(19.0, 1.0, false, [0.0; 5]),
                ],
                &["地产"],
            ),
            contract(
                "000003",
                "招商",
                &days,
                vec![
                    bar(30.0, 0.0, false, [0.03, 0.0, 0.0, 0.0, 0.0]),
                    bar(32.0, 3.0, true, [0.06, 0.0, 0.0, 0.0, 0.0]),
                    bar(31.0, 2.0, false, [0.0; 5]),
                ],
                &["沪深300", "银行"],
            ),
        ];
        DataFrame {
            start: days[0],
            end: days[2],
            index: days.to_vec(),
            list,
            sector: Arc::new(HashSet::from(["银行".to_string(), "地产".to_string()])),
            indice: Arc::new(HashSet::from(["沪深300".to_string()])),
        }
    }

    /// 单只股票 D：仅 day1/day3 有数据（day2 停牌）。
    fn frame_with_suspension() -> DataFrame {
        let days = [date(1), date(3)];
        let list = vec![contract(
            "000004",
            "停牌",
            &days,
            vec![
                bar(10.0, 0.0, false, [0.01, 0.0, 0.0, 0.0, 0.0]),
                bar(12.0, 0.0, false, [0.0; 5]),
            ],
            &["测试"],
        )];
        DataFrame {
            start: days[0],
            end: days[1],
            index: vec![date(1), date(2), date(3)],
            list,
            sector: Arc::new(HashSet::new()),
            indice: Arc::new(HashSet::new()),
        }
    }

    fn req(field: Field, direction: Direction, filter: OpFilter, select: usize) -> Req {
        chain_req(vec![Stage {
            field,
            direction,
            filter,
            select,
        }])
    }

    fn chain_req(stages: Vec<Stage>) -> Req {
        Req {
            stages,
            profit_mode: 1,
            base: PoolFilter::new(date(1), date(3)),
        }
    }

    fn codes(items: &[StockItem]) -> Vec<&str> {
        items.iter().map(|item| item.code.as_str()).collect()
    }

    // 排序与截取：Desc 取前 N 顺序正确，Asc 反转。
    #[test]
    fn select_at_orders_by_direction_and_truncates() {
        let frame = frame();
        let asc = select_at(&frame, &req(Field::TotalMarket, Direction::Asc, OpFilter::None, 2), date(1));
        assert_eq!(codes(&asc), ["000001", "000002"]);
        let desc = select_at(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 2), date(1));
        assert_eq!(codes(&desc), ["000003", "000002"]);
        assert_eq!(desc[0].factor, 30.0);
    }

    // 过滤边界：Less/Greater 严格、Equal 精确命中。
    #[test]
    fn select_at_applies_filter_boundary() {
        let frame = frame();
        let greater = select_at(&frame, &req(Field::TotalMarket, Direction::Asc, OpFilter::Greater(15.0), 10), date(1));
        assert_eq!(codes(&greater), ["000002", "000003"]);
        let less = select_at(&frame, &req(Field::TotalMarket, Direction::Asc, OpFilter::Less(25.0), 10), date(1));
        assert_eq!(codes(&less), ["000001", "000002"]);
        let equal = select_at(&frame, &req(Field::TotalMarket, Direction::Asc, OpFilter::Equal(20.0), 10), date(1));
        assert_eq!(codes(&equal), ["000002"]);
    }

    // ST 过滤：filter_st=true 时剔除当日 ST 股。
    #[test]
    fn select_at_filters_st() {
        let frame = frame();
        let mut args = req(Field::TotalMarket, Direction::Desc, OpFilter::None, 10);
        assert_eq!(select_at(&frame, &args, date(2)).len(), 3);
        args.base.filter_st = true;
        assert_eq!(codes(&select_at(&frame, &args, date(2))), ["000002", "000001"]);
    }

    // 指针映射：选中项 code/name/标签与合约元数据一致。
    #[test]
    fn select_at_maps_metadata() {
        let frame = frame();
        let desc = select_at(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 3), date(1));
        let top = &desc[0];
        assert_eq!(top.code, "000003");
        assert_eq!(top.name, "招商");
        assert_eq!(top.exchange, "上交所");
        assert_eq!(top.tags, ["沪深300", "银行"]);
        assert_eq!(top.volume, 1_000.0);
        assert!(!top.is_st);
    }

    // 停牌：当日无数据合约跳过，不入选。
    #[test]
    fn select_at_skips_suspended() {
        let frame = frame_with_suspension();
        assert!(select_at(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 10), date(2)).is_empty());
        let items = select_at(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 10), date(1));
        assert_eq!(codes(&items), ["000004"]);
    }

    // 净值累乘：首点 1.0，组合/基准按等权收益逐日累乘。
    #[test]
    fn history_compounds_portfolio_nav() {
        let frame = frame();
        let result = history(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 2));
        // 首点基线 1.0 + 三个交易日 → 4 个点
        assert_eq!(result.datetime.len(), 4);
        assert!((result.portfolio[0] - 1.0).abs() < 1e-12);
        assert!((result.benchmark[0] - 1.0).abs() < 1e-12);
        // day1：选 C(0.03)/B(0.02) → r=0.025；基准 (0.01+0.02+0.03)/3=0.02
        assert!((result.portfolio[1] - 1.025).abs() < 1e-9);
        assert!((result.benchmark[1] - 1.02).abs() < 1e-9);
        // day2：选 C(0.06)/B(0.05) → r=0.055；基准 (0.04+0.05+0.06)/3=0.05
        assert!((result.portfolio[2] - 1.025 * 1.055).abs() < 1e-9);
        assert!((result.benchmark[2] - 1.02 * 1.05).abs() < 1e-9);
        // day3 尾部 profit=0 → 净值持平
        assert!((result.portfolio[3] - result.portfolio[2]).abs() < 1e-12);
        assert!((result.benchmark[3] - result.benchmark[2]).abs() < 1e-12);
    }

    // 换手率（首日 1.0、次日按交集）与统计指标手算一致。
    #[test]
    fn history_turnover_and_stats() {
        let frame = frame();
        let result = history(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 2));
        assert_eq!(result.turnover, [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(result.count, [0, 2, 2, 2]);
        assert!((result.stats.total_profit - (1.025 * 1.055 - 1.0)).abs() < 1e-9);
        assert!((result.stats.annualized - (1.025 * 1.055 - 1.0) / 3.0 * 365.0).abs() < 1e-6);
        assert!((result.stats.max_drawdown - 0.0).abs() < 1e-12);
        assert!((result.stats.win_rate - 2.0 / 3.0).abs() < 1e-9);
    }

    // 尾部 profit=0：净值/基准末点持平且无 NaN。
    #[test]
    fn history_tail_zero_profit_no_nan() {
        let frame = frame();
        let result = history(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 2));
        assert!(result.portfolio.iter().all(|v| v.is_finite()));
        assert!(result.benchmark.iter().all(|v| v.is_finite()));
        assert!(result.turnover.iter().all(|v| v.is_finite()));
        assert!((result.portfolio[3] - result.portfolio[2]).abs() < 1e-12);
        assert!((result.benchmark[3] - result.benchmark[2]).abs() < 1e-12);
    }

    // 空名单日：收益/换手/count 记 0，日期保留，净值持平。
    #[test]
    fn history_empty_day_keeps_date_with_zero() {
        let frame = frame_with_suspension();
        let result = history(&frame, &req(Field::TotalMarket, Direction::Desc, OpFilter::None, 10));
        assert_eq!(result.datetime.len(), 4);
        assert_eq!(result.count, [0, 1, 0, 1]);
        assert_eq!(result.turnover, [0.0, 1.0, 0.0, 1.0]);
        // day2 空名单：组合/基准净值与前一交易日持平
        assert!((result.portfolio[2] - result.portfolio[1]).abs() < 1e-12);
        assert!((result.benchmark[2] - result.benchmark[1]).abs() < 1e-12);
        // day3 尾部 profit=0：继续持平
        assert!((result.portfolio[3] - result.portfolio[2]).abs() < 1e-12);
    }

    // 算子链：市值最小 2 只 → 其中收盘价最低 1 只（微盘股语义小样本）。
    #[test]
    fn select_at_applies_stage_chain() {
        let frame = frame();
        let args = chain_req(vec![
            Stage {
                field: Field::TotalMarket,
                direction: Direction::Asc,
                filter: OpFilter::None,
                select: 2,
            },
            Stage {
                field: Field::Close,
                direction: Direction::Asc,
                filter: OpFilter::None,
                select: 1,
            },
        ]);
        // day1：市值最小 2 只 = A(10)/B(20)，其中收盘最低 = A(10)
        let items = select_at(&frame, &args, date(1));
        assert_eq!(codes(&items), ["000001"]);
    }

    // 链式逐日回测：净值按链式选中股票等权累乘。
    #[test]
    fn history_applies_stage_chain_daily() {
        let frame = frame();
        let args = chain_req(vec![
            Stage {
                field: Field::TotalMarket,
                direction: Direction::Asc,
                filter: OpFilter::None,
                select: 2,
            },
            Stage {
                field: Field::Close,
                direction: Direction::Asc,
                filter: OpFilter::None,
                select: 1,
            },
        ]);
        let result = history(&frame, &args);
        // day1 选 A（市值最小2中收盘最低，profit 0.01）；day2 选 A(0.04)；day3 尾部 0
        assert_eq!(result.count, [0, 1, 1, 1]);
        assert!((result.portfolio[1] - 1.01).abs() < 1e-9);
        assert!((result.portfolio[2] - 1.01 * 1.04).abs() < 1e-9);
        assert!((result.portfolio[3] - result.portfolio[2]).abs() < 1e-12);
    }
}
