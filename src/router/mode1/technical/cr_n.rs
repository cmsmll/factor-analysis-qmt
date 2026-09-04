//! CR 指标因子（中间价动量指标）。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;
use time::Date;

use crate::{
    math::dev,
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

/// 注册 26 日 CR 指标因子。
pub async fn router() -> Router {
    MODE1.register(Arc::new(|filter| Req::register(filter, 26))).await;
    Router::with_path(Req::id())
        .post(cr_n)
        .push(Router::with_path("detail").post(cr_n_detail))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// CR 指标周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
}

impl Core {
    fn new(period: usize) -> Self {
        Self {
            period: UntArg::new("CR 周期", period),
        }
    }
}

/// CR 指标因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period),
        }
    }

    fn register(filter: &Filter, period: usize) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || cr_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// CR 指标单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 N 日 CR 指标进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_cr_n",
    responses(
        (status_code = 200, description = "CR 指标因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn cr_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || cr_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行 CR 指标目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。预热 = `core.period` 个交易日：从 `date` 前 `period`
/// 个交易日开始喂 CrWindow，保证目标日的 CR 值与主分析口径一致（CR 只依赖最近
/// `period` 个交易日的分子分母窗口）。
#[endpoint]
pub async fn cr_n_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || cr_n_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn cr_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("CR 指标{period}日"),
        format!("CR:=SUM(H-REF(MID,1),N)/SUM(REF(MID,1)-L,N)*100; MID:=(H+L+C)/3; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![CrWindow::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(factor) = store.next(curr.high, curr.low, curr.close)
            {
                items.push(Mode1Temp { factor, profit });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = `core.period` 个交易日，把
/// `date` 前 `period` 日起的整个窗口喂给 CrWindow（与主分析相同的守卫与推进顺序），
/// 仅目标日收集结果行。
fn cr_n_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period));
    let mut store = vec![CrWindow::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(factor) = store.next(curr.high, curr.low, curr.close)
            {
                if is_target {
                    rows.push(DetailRow::new(&item.metadata, curr, finance, factor, profit));
                }
            }
        }
    }
    day_value(date, count, rows)
}

/// CR 滑动累加窗口：分子 Σ(H-昨日中间价)，分母 Σ(昨日中间价-L)。
#[derive(Clone)]
struct CrWindow {
    idx: usize,
    len: usize,
    prev_mid: f64,
    sum_num: f64,
    sum_den: f64,
    buf_num: Vec<f64>,
    buf_den: Vec<f64>,
}

impl CrWindow {
    fn new(len: usize) -> Self {
        assert!(len >= 2, "CrWindow 周期 len 必须大于等于 2");
        Self {
            idx: 0,
            len,
            prev_mid: 0.0,
            sum_num: 0.0,
            sum_den: 0.0,
            buf_num: vec![0.0; len],
            buf_den: vec![0.0; len],
        }
    }

    /// 输入当日 H/L/C，返回 CR 值；预热未完成时返回 `None`。
    fn next(&mut self, high: f64, low: f64, close: f64) -> Option<f64> {
        let pos = self.idx % self.len;
        let mid = (high + low + close) / 3.0;

        if self.idx >= 1 {
            let num = high - self.prev_mid;
            let den = self.prev_mid - low;
            self.sum_num += num - self.buf_num[pos];
            self.sum_den += den - self.buf_den[pos];
            self.buf_num[pos] = num;
            self.buf_den[pos] = den;
        }
        self.prev_mid = mid;
        self.idx += 1;

        if self.idx < self.len {
            return None;
        }

        Some(dev(self.sum_num, self.sum_den) * 100.0)
    }
}
