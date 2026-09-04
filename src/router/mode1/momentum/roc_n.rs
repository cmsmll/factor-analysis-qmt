//! 变动速率因子（Price Rate of Change）。

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

/// 注册 6/12/60/120 日变动速率因子。
pub async fn router() -> Router {
    for period in [6, 12, 60, 120] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, period))).await;
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(roc_n)
            .push(Router::with_path("detail").post(roc_n_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 变动速率周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
}

impl Core {
    fn new(period: usize) -> Self {
        Self {
            period: UntArg::new("变动速率周期", period),
        }
    }
}

/// 变动速率因子分析请求。
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
        let recv = MODE1.cache.get_or_run(key, move || roc_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 变动速率因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 N 日变动速率进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_roc_n",
    responses(
        (status_code = 200, description = "变动速率因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn roc_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || roc_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn roc_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("变动速率因子(ROC){period}日"),
        format!("ROC:=(CLOSE-REF(CLOSE,N))/REF(CLOSE,N)*100; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        for item in &df.list {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(prev) = item.before(&index, period)
            {
                items.push(Mode1Temp {
                    factor: dev(curr.close - prev.close, prev.close) * 100.0,
                    profit,
                });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}
/// 执行变动速率因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。变动速率因子无预热需求，直接取目标日当天全市场分位明细；
/// 需要 N 个交易日前收盘价计算当日 ROC，目标日不处于可回看位置时按当日口径跳过该股。
#[endpoint]
pub async fn roc_n_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || roc_n_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 计算目标日单日分位明细：变动速率无预热需求（warmup = 0），
/// 周期前收盘价通过 `before` 回看合约自身完整行情获得。
fn roc_n_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, 0));
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        for item in &df.list {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(prev) = item.before(&index, period)
            {
                rows.push(DetailRow::new(
                    &item.metadata,
                    curr,
                    finance,
                    dev(curr.close - prev.close, prev.close) * 100.0,
                    profit,
                ));
            }
        }
    }
    day_value(date, count, rows)
}
