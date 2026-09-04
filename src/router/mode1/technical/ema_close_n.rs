//! 收盘价指数移动均线因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;
use time::Date;

use crate::{
    math::{EMA, dev},
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{
        Base,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
    },
    toolbox::VJson,
};

/// 注册 10 日和 20 日收盘价指数移动均线因子。
pub async fn router() -> Router {
    MODE1.register(Arc::new(|filter| Req::register(filter, 10))).await;
    MODE1.register(Arc::new(|filter| Req::register(filter, 20))).await;
    Router::new().push(
        Router::with_path(Req::id())
            .post(ema_close_n)
            .push(Router::with_path("detail").post(ema_close_n_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 计算周期，单位为交易日。
    #[validate(custom(function = "super::validate_period"))]
    pub period: UntArg,
}

impl Core {
    pub fn new(period: usize) -> Self {
        assert!(period >= 2, "周期必须大于等于 2");
        Self {
            period: UntArg::new("多日周期", period),
        }
    }
}

/// 收盘价指数移动均线因子分析请求。
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
        let recv = MODE1.cache.get_or_run(key, move || ema_close_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 收盘价指数移动均线因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 N 日收盘价指数移动均线与今日收盘价的比值进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_ema_close_n",
    responses(
        (status_code = 200, description = "收盘价指数移动均线因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn ema_close_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || ema_close_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行收盘价指数移动均线因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。预热 = 500 个交易日：EMA 为递归平滑，从 `date` 前 500
/// 个交易日开始喂入即可充分收敛，保证目标日 EMA 与主分析口径一致（500 日远超
/// 周期 `period` 的收敛余量）。
#[endpoint]
pub async fn ema_close_n_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || ema_close_n_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn ema_close_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("指数移动均线因子(EMA){period}日"),
        format!("EMA_CLOSE_N:=EMA(CLOSE,N)/CLOSE; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![EMA::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(avg_close) = store.next(curr.close)
            {
                items.push(Mode1Temp {
                    factor: dev(avg_close, curr.close),
                    profit,
                });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = 500 个交易日，把 `date` 前 500 日起的窗口
/// 喂给 EMA（与主分析相同的守卫与推进顺序，让 EMA 充分收敛），仅目标日收集结果行。
fn ema_close_n_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, 500));
    let mut store = vec![EMA::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(avg_close) = store.next(curr.close)
            {
                if is_target {
                    rows.push(DetailRow::new(
                        &item.metadata,
                        curr,
                        finance,
                        dev(avg_close, curr.close),
                        profit,
                    ));
                }
            }
        }
    }
    day_value(date, count, rows)
}
