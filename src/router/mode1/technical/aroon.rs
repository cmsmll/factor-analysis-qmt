//! Aroon 指标上/下轨因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;
use tokio::sync::broadcast::Receiver;

use crate::{
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{
        Base, validate_period,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
    },
    toolbox::VJson,
};

/// Aroon 轨道：上轨 / 下轨。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum AroonBand {
    /// 上轨：基于窗口内最高价位置
    Upper,
    /// 下轨：基于窗口内最低价位置
    Lower,
}

impl AroonBand {
    fn label(self) -> &'static str {
        match self {
            Self::Upper => "上轨",
            Self::Lower => "下轨",
        }
    }
}

/// 注册 25 日 Aroon 上/下轨因子。
pub async fn router() -> Router {
    for band in [AroonBand::Upper, AroonBand::Lower] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, 25, band))).await;
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(aroon)
            .push(Router::with_path("detail").post(aroon_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// Aroon 周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
    /// 轨道类型。
    pub band: AroonBand,
}

impl Core {
    fn new(period: usize, band: AroonBand) -> Self {
        Self {
            period: UntArg::new("Aroon 周期", period),
            band,
        }
    }
}

/// Aroon 因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize, band: AroonBand) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period, band),
        }
    }

    fn register(filter: &Filter, period: usize, band: AroonBand) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period, band);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || aroon_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// Aroon 因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct DetailReq {
    #[serde(flatten)]
    #[validate(nested)]
    req: Req,
    /// 目标日期 `YYYY-MM-DD`
    #[serde(default, with = "crate::toolbox::serde::date_format::opt")]
    date: Option<Date>,
}

impl ArgsHandle for DetailReq {}

/// 按 N 日 Aroon 轨道值进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_aroon",
    responses(
        (status_code = 200, description = "Aroon 因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn aroon(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || aroon_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行 Aroon 因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。预热 = `core.period` 个交易日：从 `date` 前 `period` 个交易日
/// 开始喂 Aroon 窗口，保证目标日的轨道值与主分析口径一致。
#[endpoint]
pub async fn aroon_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || aroon_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn aroon_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let band = args.core.band;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("Aroon{}{}因子", band.label(), period),
        format!("AROON:=(N-距窗口最高/最低天数)/N*100; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![AroonWindow::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(factor) = store.next(curr.high, curr.low).map(|(up, down)| match band {
                    AroonBand::Upper => up,
                    AroonBand::Lower => down,
                })
            {
                items.push(Mode1Temp { factor, profit });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = `period` 个交易日，推进窗口后仅收集目标日当天的 Aroon 值。
fn aroon_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let band = args.req.core.band;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period));
    let mut store = vec![AroonWindow::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(factor) = store.next(curr.high, curr.low).map(|(up, down)| match band {
                    AroonBand::Upper => up,
                    AroonBand::Lower => down,
                })
            {
                if is_target {
                    rows.push(DetailRow::new(&item.metadata, curr, finance, factor, profit));
                }
            }
        }
    }
    day_value(date, count, rows)
}

/// Aroon 窗口：统计 N 日内最高/最低价距当前的天数，输出 (AroonUp, AroonDown)。
#[derive(Clone)]
struct AroonWindow {
    idx: usize,
    len: usize,
    buf: Vec<(f64, f64)>,
}

impl AroonWindow {
    fn new(len: usize) -> Self {
        assert!(len >= 2, "AroonWindow 周期 len 必须大于等于 2");
        Self {
            idx: 0,
            len,
            buf: vec![(0.0, 0.0); len],
        }
    }

    fn next(&mut self, high: f64, low: f64) -> Option<(f64, f64)> {
        let pos = self.idx % self.len;
        self.buf[pos] = (high, low);
        self.idx += 1;

        if self.idx < self.len {
            return None;
        }

        // 遍历窗口找出最高/最低价距当前的天数（0 = 当日）
        let n = self.len as f64;
        let mut hi_days = 0usize;
        let mut lo_days = 0usize;
        let mut hi = f64::NEG_INFINITY;
        let mut lo = f64::INFINITY;
        for back in 0..self.len {
            let index = (self.idx - 1 - back) % self.len;
            let (h, l) = self.buf[index];
            if h >= hi {
                hi = h;
                hi_days = back;
            }
            if l <= lo {
                lo = l;
                lo_days = back;
            }
        }

        Some((
            (n - hi_days as f64) / n * 100.0,
            (n - lo_days as f64) / n * 100.0,
        ))
    }
}
