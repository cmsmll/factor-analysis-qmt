//! 资金流量指标（Money Flow Index）因子。

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
        Base,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
        validate_period,
    },
    toolbox::VJson,
};

/// 注册 14 日资金流量指标因子。
pub async fn router() -> Router {
    MODE1.register(Arc::new(|filter| Req::register(filter, 14))).await;
    Router::with_path(Req::id())
        .post(mfi_n)
        .push(Router::with_path("detail").post(mfi_n_detail))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// MFI 周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
}

impl Core {
    fn new(period: usize) -> Self {
        Self {
            period: UntArg::new("MFI 周期", period),
        }
    }
}

/// 资金流量指标因子分析请求。
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
        let recv = MODE1.cache.get_or_run(key, move || mfi_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 资金流量指标因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 N 日资金流量指标进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_mfi_n",
    responses(
        (status_code = 200, description = "资金流量指标因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn mfi_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || mfi_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行资金流量指标因子目标日单日分位明细查询。
///
/// 预热 = `core.period` 个交易日：从 `date` 前 `period` 个交易日开始推进 MfiWindow，
/// 每日守卫（行情/ST/前一日收盘）与主分析完全一致，保证目标日 MFI 口径相同。
#[endpoint]
pub async fn mfi_n_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || mfi_n_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn mfi_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("资金流量指标(MFI){period}日"),
        format!("TYP:=(H+L+C)/3; MF:=TYP*VOLUME; MFI:=100-100/(1+Σ正向MF/Σ负向MF); N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![MfiWindow::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = store.next(curr, prev.close)
            {
                items.push(Mode1Temp { factor, profit });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = `core.period` 个交易日，从目标日回推 `period` 个交易日
/// 推进 MfiWindow（守卫含前一日收盘判断），仅收集目标日当天分位行。
fn mfi_n_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period));
    let mut store = vec![MfiWindow::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = store.next(curr, prev.close)
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

/// MFI 滑动窗口：按当日典型价相对前收的方向累计正/负资金流。
#[derive(Clone)]
struct MfiWindow {
    idx: usize,
    len: usize,
    pos_sum: f64,
    neg_sum: f64,
    buf_pos: Vec<f64>,
    buf_neg: Vec<f64>,
}

impl MfiWindow {
    fn new(len: usize) -> Self {
        assert!(len >= 2, "MfiWindow 周期 len 必须大于等于 2");
        Self {
            idx: 0,
            len,
            pos_sum: 0.0,
            neg_sum: 0.0,
            buf_pos: vec![0.0; len],
            buf_neg: vec![0.0; len],
        }
    }

    fn next(&mut self, market: &Market, prev_close: f64) -> Option<f64> {
        let pos = self.idx % self.len;
        let typ = (market.high + market.low + market.close) / 3.0;
        let flow = typ * market.volume;
        let (pos_flow, neg_flow) = if typ > prev_close {
            (flow, 0.0)
        } else if typ < prev_close {
            (0.0, flow)
        } else {
            (0.0, 0.0)
        };

        self.pos_sum += pos_flow - self.buf_pos[pos];
        self.neg_sum += neg_flow - self.buf_neg[pos];
        self.buf_pos[pos] = pos_flow;
        self.buf_neg[pos] = neg_flow;
        self.idx += 1;

        if self.idx < self.len {
            return None;
        }

        // MFI = 100 - 100 / (1 + 正向/负向)
        let ratio = dev(self.pos_sum, self.neg_sum);
        Some(100.0 - 100.0 / (1.0 + ratio))
    }
}
