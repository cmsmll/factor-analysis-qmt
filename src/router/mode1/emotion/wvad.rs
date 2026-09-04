//! WVAD 系量价因子：WVAD 6 日均值、20 日资金流量、威廉变异离散量。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;
use tokio::sync::broadcast::Receiver;

use crate::{
    math::{SMA, dev},
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{
        Base,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
        validate_period,
    },
    toolbox::VJson,
};

/// WVAD 系指标类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum WvadKind {
    /// WVAD 6 日均值
    Wvad6,
    /// 20 日资金流量（WVAD 20 日累计）
    Wvad20,
    /// 威廉变异离散量（WVAD 24 日累计）
    Williams,
}

impl WvadKind {
    fn label(self) -> &'static str {
        match self {
            Self::Wvad6 => "WVAD6 日均值",
            Self::Wvad20 => "20 日资金流量",
            Self::Williams => "威廉变异离散量",
        }
    }

    fn window(self) -> usize {
        match self {
            Self::Wvad6 => 6,
            Self::Wvad20 => 20,
            Self::Williams => 24,
        }
    }
}

/// 注册 WVAD 6 日均值 / 20 日资金流量 / 威廉变异离散量因子。
pub async fn router() -> Router {
    for kind in [WvadKind::Wvad6, WvadKind::Wvad20, WvadKind::Williams] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, kind.window(), kind))).await;
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(wvad)
            .push(Router::with_path("detail").post(wvad_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// WVAD 系周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
    /// 指标类型。
    pub kind: WvadKind,
}

impl Core {
    fn new(period: usize, kind: WvadKind) -> Self {
        Self {
            period: UntArg::new("WVAD 周期", period),
            kind,
        }
    }
}

/// WVAD 系因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize, kind: WvadKind) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period, kind),
        }
    }

    fn register(filter: &Filter, period: usize, kind: WvadKind) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period, kind);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || wvad_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// WVAD 系因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 WVAD 系指标进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_wvad",
    responses(
        (status_code = 200, description = "WVAD 系因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn wvad(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || wvad_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行 WVAD 系因子目标日单日分位明细查询。
///
/// 预热 = `kind.window().max(6)` 个交易日：从 `date` 前该数开始推进状态，
/// Wvad6 用 SMA(6)、Wvad20/Williams 用 WvadSum(kind.window())，与主分析口径一致。
#[endpoint]
pub async fn wvad_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || wvad_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn wvad_run(args: Req) -> Box<RawValue> {
    let kind = args.core.kind;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        kind.label(),
        format!("WVAD:=Σ((C-O)/(H-L)*V); {}; N:={}", kind.label(), args.core.period.value),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());

    // Wvad6 用 SMA 平滑单日 WVAD；Wvad20/Williams 用滑动累计
    let mut avg_store = vec![SMA::new(6); df.list.len()];
    let mut sum_store = vec![WvadSum::new(kind.window()); df.list.len()];

    for index in df.index_iter() {
        for (item, (avg_store, sum_store)) in df.list.iter().zip(avg_store.iter_mut().zip(sum_store.iter_mut())) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
            {
                let daily = dev(curr.close - curr.open, curr.high - curr.low) * curr.volume;
                let factor = match kind {
                    WvadKind::Wvad6 => avg_store.next(daily),
                    WvadKind::Wvad20 | WvadKind::Williams => sum_store.next(daily),
                };
                if let Some(factor) = factor {
                    items.push(Mode1Temp { factor, profit });
                }
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = `kind.window().max(6)` 个交易日（Wvad6 的 SMA(6) 需至少 6 日，
/// Wvad20/Williams 的 WvadSum 需窗口长度），仅收集目标日当天分位行。
fn wvad_detail_run(args: DetailReq) -> Box<RawValue> {
    let kind = args.req.core.kind;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, kind.window().max(6)));
    let mut avg_store = vec![SMA::new(6); df.list.len()];
    let mut sum_store = vec![WvadSum::new(kind.window()); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, (avg_store, sum_store)) in df.list.iter().zip(avg_store.iter_mut().zip(sum_store.iter_mut())) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
            {
                let daily = dev(curr.close - curr.open, curr.high - curr.low) * curr.volume;
                let factor = match kind {
                    WvadKind::Wvad6 => avg_store.next(daily),
                    WvadKind::Wvad20 | WvadKind::Williams => sum_store.next(daily),
                };
                if let Some(factor) = factor {
                    if is_target {
                        rows.push(DetailRow::new(&item.metadata, curr, finance, factor, profit));
                    }
                }
            }
        }
    }
    day_value(date, count, rows)
}

/// WVAD 滑动累计窗口。
#[derive(Clone)]
struct WvadSum {
    idx: usize,
    len: usize,
    sum: f64,
    buf: Vec<f64>,
}

impl WvadSum {
    fn new(len: usize) -> Self {
        assert!(len >= 2, "WvadSum 周期 len 必须大于等于 2");
        Self {
            idx: 0,
            len,
            sum: 0.0,
            buf: vec![0.0; len],
        }
    }

    fn next(&mut self, value: f64) -> Option<f64> {
        let pos = self.idx % self.len;
        let old = self.buf[pos];
        self.buf[pos] = value;
        self.sum += value - old;
        self.idx += 1;

        (self.idx >= self.len).then_some(self.sum)
    }
}
