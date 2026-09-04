//! 心理线指标（Psychological Line）。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;
use time::Date;

use crate::{
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

/// 注册 12 日心理线因子。
pub async fn router() -> Router {
    MODE1.register(Arc::new(|filter| Req::register(filter, 12))).await;
    Router::with_path(Req::id())
        .post(psy_n)
        .push(Router::with_path("detail").post(psy_n_detail))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 心理线周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
}

impl Core {
    fn new(period: usize) -> Self {
        Self {
            period: UntArg::new("心理线周期", period),
        }
    }
}

/// 心理线因子分析请求。
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
        let recv = MODE1.cache.get_or_run(key, move || psy_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 心理线因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 N 日心理线（上涨天数占比）进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_psy_n",
    responses(
        (status_code = 200, description = "心理线因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn psy_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || psy_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行心理线因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。预热 = `core.period` 个交易日：从 `date` 前 `period`
/// 个交易日开始喂 PsyCounter（上涨/下跌 0/1 计数环），保证目标日的心理线与主分析
/// 口径一致（只统计最近 `period` 个交易日）。
#[endpoint]
pub async fn psy_n_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || psy_n_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn psy_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("心理线因子(PSY){period}日"),
        format!("PSY:=COUNT(CLOSE>REF(CLOSE,1),N)/N*100; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    // 每只股票维护一个上涨计数环（0/1 入窗）
    let mut store = vec![PsyCounter::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = store.next(curr.close > prev.close)
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
/// `date` 前 `period` 日起的整个窗口喂给 PsyCounter（与主分析相同的守卫与推进顺序，
/// 含前一日收盘比较），仅目标日收集结果行。
fn psy_n_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period));
    let mut store = vec![PsyCounter::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = store.next(curr.close > prev.close)
            {
                if is_target {
                    rows.push(DetailRow::new(&item.metadata, curr, finance, factor, profit));
                }
            }
        }
    }
    day_value(date, count, rows)
}

/// 心理线计数环：统计窗口内上涨天数占比。
#[derive(Clone)]
struct PsyCounter {
    idx: usize,
    len: usize,
    count: usize,
    buf: Vec<bool>,
}

impl PsyCounter {
    fn new(len: usize) -> Self {
        assert!(len >= 2, "PsyCounter 周期 len 必须大于等于 2");
        Self {
            idx: 0,
            len,
            count: 0,
            buf: vec![false; len],
        }
    }

    fn next(&mut self, up: bool) -> Option<f64> {
        let pos = self.idx % self.len;
        if self.buf[pos] {
            self.count -= 1;
        }
        self.buf[pos] = up;
        if up {
            self.count += 1;
        }
        self.idx += 1;

        (self.idx >= self.len).then_some(self.count as f64 / self.len as f64 * 100.0)
    }
}
