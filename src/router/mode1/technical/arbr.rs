//! 人气指标（AR）、意愿指标（BR）与 ARBR 组合因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;
use tokio::sync::broadcast::Receiver;

use crate::{
    math::dev,
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{
        Base, validate_period,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
    },
    toolbox::VJson,
};

/// 指标类型：AR / BR / ARBR。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum ArbrKind {
    /// 人气指标 AR
    Ar,
    /// 意愿指标 BR
    Br,
    /// ARBR 组合
    Arbr,
}

impl ArbrKind {
    fn label(self) -> &'static str {
        match self {
            Self::Ar => "人气指标(AR)",
            Self::Br => "意愿指标(BR)",
            Self::Arbr => "ARBR",
        }
    }
}

/// 注册 26 日 AR/BR/ARBR 因子。
pub async fn router() -> Router {
    for kind in [ArbrKind::Ar, ArbrKind::Br, ArbrKind::Arbr] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, 26, kind))).await;
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(arbr)
            .push(Router::with_path("detail").post(arbr_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 统计周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
    /// 指标类型。
    pub kind: ArbrKind,
}

impl Core {
    fn new(period: usize, kind: ArbrKind) -> Self {
        Self {
            period: UntArg::new("统计周期", period),
            kind,
        }
    }
}

/// AR/BR/ARBR 因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize, kind: ArbrKind) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period, kind),
        }
    }

    fn register(filter: &Filter, period: usize, kind: ArbrKind) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period, kind);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || arbr_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// AR/BR/ARBR 因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 AR/BR/ARBR 值进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_arbr",
    responses(
        (status_code = 200, description = "AR/BR/ARBR 因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn arbr(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || arbr_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行 AR/BR/ARBR 因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。预热 = `core.period` 个交易日：从 `date` 前 `period` 个交易日
/// 开始喂 AR/BR 累加窗口，保证目标日的指标值与主分析口径一致。
#[endpoint]
pub async fn arbr_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || arbr_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn arbr_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let kind = args.core.kind;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("{}{}日", kind.label(), period),
        format!("AR:=SUM(H-O,N)/SUM(O-L,N)*100; BR:=SUM(H-REF(C,1),N)/SUM(REF(C,1)-L,N)*100; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![ArbrWindow::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = store.next(curr, prev.close).map(|(ar, br)| match kind {
                    ArbrKind::Ar => ar,
                    ArbrKind::Br => br,
                    ArbrKind::Arbr => dev(ar + br, 200.0) * 100.0,
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

/// 计算目标日单日分位明细：预热 = `period` 个交易日，推进窗口后仅收集目标日当天的 AR/BR/ARBR 值。
fn arbr_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let kind = args.req.core.kind;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period));
    let mut store = vec![ArbrWindow::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = store.next(curr, prev.close).map(|(ar, br)| match kind {
                    ArbrKind::Ar => ar,
                    ArbrKind::Br => br,
                    ArbrKind::Arbr => dev(ar + br, 200.0) * 100.0,
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

use crate::db::Market;

/// AR/BR 滑动累加窗口。
#[derive(Clone)]
struct ArbrWindow {
    idx: usize,
    len: usize,
    ar_sum_num: f64,
    ar_sum_den: f64,
    br_sum_num: f64,
    br_sum_den: f64,
    buf_ar_num: Vec<f64>,
    buf_ar_den: Vec<f64>,
    buf_br_num: Vec<f64>,
    buf_br_den: Vec<f64>,
}

impl ArbrWindow {
    fn new(len: usize) -> Self {
        assert!(len >= 2, "ArbrWindow 周期 len 必须大于等于 2");
        Self {
            idx: 0,
            len,
            ar_sum_num: 0.0,
            ar_sum_den: 0.0,
            br_sum_num: 0.0,
            br_sum_den: 0.0,
            buf_ar_num: vec![0.0; len],
            buf_ar_den: vec![0.0; len],
            buf_br_num: vec![0.0; len],
            buf_br_den: vec![0.0; len],
        }
    }

    /// 输入当日行情与前收盘，返回 `(AR, BR)`；预热未完成时返回 `None`。
    fn next(&mut self, market: &Market, prev_close: f64) -> Option<(f64, f64)> {
        let pos = self.idx % self.len;
        let ar_num = market.high - market.open;
        let ar_den = market.open - market.low;
        let br_num = market.high - prev_close;
        let br_den = prev_close - market.low;

        self.ar_sum_num += ar_num - self.buf_ar_num[pos];
        self.ar_sum_den += ar_den - self.buf_ar_den[pos];
        self.br_sum_num += br_num - self.buf_br_num[pos];
        self.br_sum_den += br_den - self.buf_br_den[pos];
        self.buf_ar_num[pos] = ar_num;
        self.buf_ar_den[pos] = ar_den;
        self.buf_br_num[pos] = br_num;
        self.buf_br_den[pos] = br_den;
        self.idx += 1;

        if self.idx < self.len {
            return None;
        }

        Some((
            dev(self.ar_sum_num, self.ar_sum_den) * 100.0,
            dev(self.br_sum_num, self.br_sum_den) * 100.0,
        ))
    }
}
