use std::cmp::Ordering;

use crate::db::Bar;
use serde::{Deserialize, Serialize};

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    /// 升序（小 → 大）
    Asc,
    /// 降序（大 → 小）
    Desc,
}

/// 因子过滤条件，作用于排序后的因子字段。
///
/// `Less`/`Greater` 为严格比较（不含边界值）；`Equal` 为精确浮点相等，
/// 对原始价格数据安全，对计算产生的值可能因浮点误差不命中。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Filter {
    /// 不过滤
    None,
    /// 字段值 < 阈值（严格）
    Less(f64),
    /// 字段值 == 阈值（精确相等）
    Equal(f64),
    /// 字段值 > 阈值（严格）
    Greater(f64),
}

/// 因子字段。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Field {
    /// 收盘价
    Close,
    /// 股息率（百分比）
    DividendYield,
    /// 总市值（单位：元）
    TotalMarket,
}

/// 因子算子：对行情切片执行「排序 → 过滤 → 截取」。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operator {
    /// 因子字段
    pub field: Field,
    /// 过滤因子
    pub filter: Filter,
    /// 挑选数量（`None` = 全部保留）
    pub select: Option<usize>,
    /// 排序方向
    pub direction: Direction,
}

impl Operator {
    /// 标准执行链路：先排序，再过滤并截取前 N 条。
    ///
    /// 返回输入切片的子切片，与原数组共享存储，零分配。
    ///
    /// 性能优化：仅截取（`filter = None` 且 `select = Some(k)`）时走部分选择
    /// `select_nth_unstable_by`（O(n)）并对选中段排序（O(k log k)），避免全排序
    /// （O(n log n)）；含过滤条件时二分依赖全序，维持全排序。
    pub fn run<'a, 'b>(&self, bars: &'a mut [&'b Bar]) -> &'a mut [&'b Bar] {
        match (self.filter, self.select) {
            // 只需前 k 条：部分选择 + 仅对选中段排序
            (Filter::None, Some(k)) => {
                let k = k.min(bars.len());
                if k == 0 {
                    return &mut bars[..0];
                }
                if k < bars.len() {
                    bars.select_nth_unstable_by(k, |&a, &b| self.cmp_field(a, b));
                    bars[..k].sort_unstable_by(|&a, &b| self.cmp_field(a, b));
                    &mut bars[..k]
                } else {
                    self.sort(bars)
                }
            }
            // 不过滤不截取：仅排序，维持输出有序契约
            (Filter::None, None) => self.sort(bars),
            // 过滤依赖有序二分：全排序后进入 select
            _ => {
                let bars = self.sort(bars);
                self.select(bars)
            }
        }
    }

    /// 按过滤条件截取前 N 条。
    ///
    /// 前置条件：`bars` 必须已按 `self.direction` 排序，否则二分结果未定义
    /// （debug 构建会断言）。`Less`/`Greater` 严格比较，`Equal` 精确匹配。
    pub fn select<'a, 'b>(&self, bars: &'a mut [&'b Bar]) -> &'a mut [&'b Bar] {
        if !matches!(self.filter, Filter::None) {
            debug_assert!(
                bars.windows(2).all(|w| self.cmp_field(w[0], w[1]).is_le()),
                "select: 输入必须已按 direction 排序"
            );
        }

        // 第一步：根据 filter，二分切出符合条件的子切片
        let filtered_slice: &'a mut [&'b Bar] = match self.filter {
            Filter::None => bars,
            Filter::Less(threshold) => match self.direction {
                Direction::Asc => {
                    // 升序：跳过 < threshold，[0..pos] 全部 < threshold
                    let pos = bars.partition_point(|bar| {
                        self.get_field(bar).total_cmp(&threshold).is_lt()
                    });
                    &mut bars[0..pos]
                }
                Direction::Desc => {
                    // 降序：跳过 >= threshold，[pos..] 全部 < threshold
                    let pos = bars.partition_point(|bar| {
                        matches!(
                            self.get_field(bar).total_cmp(&threshold),
                            Ordering::Greater | Ordering::Equal
                        )
                    });
                    &mut bars[pos..]
                }
            },
            Filter::Greater(threshold) => match self.direction {
                Direction::Asc => {
                    // 升序：跳过 <= threshold，[pos..] 全部 > threshold
                    let pos = bars.partition_point(|bar| {
                        matches!(
                            self.get_field(bar).total_cmp(&threshold),
                            Ordering::Less | Ordering::Equal
                        )
                    });
                    &mut bars[pos..]
                }
                Direction::Desc => {
                    // 降序：跳过 > threshold，[0..pos] 全部 > threshold
                    let pos = bars.partition_point(|bar| {
                        self.get_field(bar).total_cmp(&threshold).is_gt()
                    });
                    &mut bars[0..pos]
                }
            },
            Filter::Equal(threshold) => match self.direction {
                Direction::Asc => {
                    // left = 首个 >= threshold（跳过 < threshold）
                    // right = 首个 > threshold（跳过 <= threshold）
                    let left = bars.partition_point(|bar| {
                        self.get_field(bar).total_cmp(&threshold).is_lt()
                    });
                    let right = bars.partition_point(|bar| {
                        self.get_field(bar).total_cmp(&threshold).is_le()
                    });
                    &mut bars[left..right]
                }
                Direction::Desc => {
                    // left = 首个 <= threshold（跳过 > threshold）
                    // right = 首个 < threshold（跳过 >= threshold）
                    let left = bars.partition_point(|bar| {
                        self.get_field(bar).total_cmp(&threshold).is_gt()
                    });
                    let right = bars.partition_point(|bar| {
                        self.get_field(bar).total_cmp(&threshold).is_ge()
                    });
                    &mut bars[left..right]
                }
            },
        };

        // 第二步：按 select 数量截取前 N 条
        let end = match self.select {
            Some(n) => n.min(filtered_slice.len()),
            None => filtered_slice.len(),
        };
        &mut filtered_slice[0..end]
    }

    /// 按 `self.direction` 原地排序（不稳定排序）。
    ///
    /// 相等元素的相对顺序不保证；如需跨版本可复现的平局边界，请另加次级比较键。
    pub fn sort<'a, 'b>(&self, bars: &'a mut [&'b Bar]) -> &'a mut [&'b Bar] {
        bars.sort_unstable_by(|&a, &b| self.cmp_field(a, b));
        bars
    }

    /// 提取因子字段值。
    #[inline]
    pub fn get_field(&self, bar: &Bar) -> f64 {
        match self.field {
            Field::Close => bar.market.close,
            Field::DividendYield => bar.finance.dividend_yield,
            Field::TotalMarket => bar.finance.total_market,
        }
    }

    /// 因子字段比较器（按 `self.direction`），供排序与部分选择复用。
    #[inline]
    fn cmp_field(&self, a: &Bar, b: &Bar) -> Ordering {
        let (left, right) = (self.get_field(a), self.get_field(b));
        match self.direction {
            Direction::Asc => left.total_cmp(&right),
            Direction::Desc => right.total_cmp(&left),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Finance, Market};
    use time::Date;

    fn bar(close: f64) -> Bar {
        Bar {
            market: Market {
                datetime: Date::parse(
                    "2025-01-01",
                    &time::format_description::well_known::Iso8601::DATE,
                )
                .unwrap(),
                change_percent: 0.0,
                open: close,
                close,
                high: close,
                low: close,
                volume: 0.0,
                amount: 0.0,
                turnover: 0.0,
                is_st: false,
            },
            finance: Finance { total_market: 0.0, dividend_yield: 0.0 },
            profit: [0.0; 5],
        }
    }

    fn op(filter: Filter, select: Option<usize>, direction: Direction) -> Operator {
        Operator {
            field: Field::Close,
            filter,
            select,
            direction,
        }
    }

    /// 以乱序输入运行算子，返回结果中的 close 序列。
    fn run_closes(op: &Operator, closes: &[f64]) -> Vec<f64> {
        let owned: Vec<Bar> = closes.iter().map(|&c| bar(c)).collect();
        let mut refs: Vec<&Bar> = owned.iter().collect();
        op.run(&mut refs)
            .iter()
            .map(|b| b.market.close)
            .collect()
    }

    // 乱序输入 [5,8,1,5,3,10,5]：Asc 排序 [1,3,5,5,5,8,10]，Desc 排序 [10,8,5,5,5,3,1]。
    const UNSORTED: [f64; 7] = [5.0, 8.0, 1.0, 5.0, 3.0, 10.0, 5.0];
    #[test]
    fn field_reads_close_and_finance_values() {
        let mut owned = bar(3.5);
        owned.finance.total_market = 1_000.0;
        owned.finance.dividend_yield = 2.5;

        let op = |field| Operator {
            field,
            filter: Filter::None,
            select: None,
            direction: Direction::Asc,
        };
        assert_eq!(op(Field::Close).get_field(&owned), 3.5);
        assert_eq!(op(Field::TotalMarket).get_field(&owned), 1_000.0);
        assert_eq!(op(Field::DividendYield).get_field(&owned), 2.5);
    }

    // 按财务字段排序：TotalMarket 降序取前 2，验证 get_field 接入整条链路。
    #[test]
    fn run_orders_by_finance_field() {
        let mut owned: Vec<Bar> = vec![bar(1.0), bar(2.0), bar(3.0)];
        owned[0].finance.total_market = 300.0;
        owned[1].finance.total_market = 100.0;
        owned[2].finance.total_market = 200.0;
        let mut refs: Vec<&Bar> = owned.iter().collect();
        let op = Operator {
            field: Field::TotalMarket,
            filter: Filter::None,
            select: Some(2),
            direction: Direction::Desc,
        };
        let result = op.run(&mut refs);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].finance.total_market, 300.0);
        assert_eq!(result[1].finance.total_market, 200.0);
    }

    // Equal 回归：修复前两个方向均恒返回空切片。
    #[test]
    fn filter_equal_asc_returns_matching_slice() {
        assert_eq!(
            run_closes(&op(Filter::Equal(5.0), None, Direction::Asc), &UNSORTED),
            vec![5.0, 5.0, 5.0]
        );
    }

    #[test]
    fn filter_equal_desc_returns_matching_slice() {
        assert_eq!(
            run_closes(&op(Filter::Equal(5.0), None, Direction::Desc), &UNSORTED),
            vec![5.0, 5.0, 5.0]
        );
    }

    // Less/Greater 边界：严格比较，不含等值。
    #[test]
    fn filter_less_boundary_excludes_equal() {
        assert_eq!(
            run_closes(&op(Filter::Less(5.0), None, Direction::Asc), &UNSORTED),
            vec![1.0, 3.0]
        );
        assert_eq!(
            run_closes(&op(Filter::Less(5.0), None, Direction::Desc), &UNSORTED),
            vec![3.0, 1.0]
        );
    }

    #[test]
    fn filter_greater_boundary_excludes_equal() {
        assert_eq!(
            run_closes(&op(Filter::Greater(5.0), None, Direction::Asc), &UNSORTED),
            vec![8.0, 10.0]
        );
        assert_eq!(
            run_closes(&op(Filter::Greater(5.0), None, Direction::Desc), &UNSORTED),
            vec![10.0, 8.0]
        );
    }

    // filter=None + select=Some：走部分选择路径，乱序输入同样取前 N。
    #[test]
    fn select_truncates_to_n() {
        assert_eq!(
            run_closes(&op(Filter::None, Some(2), Direction::Desc), &UNSORTED),
            vec![10.0, 8.0]
        );
        assert_eq!(
            run_closes(&op(Filter::None, Some(0), Direction::Desc), &UNSORTED),
            Vec::<f64>::new()
        );
        // select 超过长度：保留全部（k >= len 走全排序分支）
        assert_eq!(
            run_closes(&op(Filter::None, Some(99), Direction::Asc), &UNSORTED),
            vec![1.0, 3.0, 5.0, 5.0, 5.0, 8.0, 10.0]
        );
    }

    // 整条链路：排序 → 过滤 → 截取。
    #[test]
    fn run_sorts_then_filters_then_truncates() {
        assert_eq!(
            run_closes(&op(Filter::Less(5.0), Some(1), Direction::Asc), &UNSORTED),
            vec![1.0]
        );
    }

    #[test]
    fn direction_orders_output() {
        assert_eq!(
            run_closes(&op(Filter::None, None, Direction::Asc), &UNSORTED),
            vec![1.0, 3.0, 5.0, 5.0, 5.0, 8.0, 10.0]
        );
        assert_eq!(
            run_closes(&op(Filter::None, None, Direction::Desc), &UNSORTED),
            vec![10.0, 8.0, 5.0, 5.0, 5.0, 3.0, 1.0]
        );
    }
}
