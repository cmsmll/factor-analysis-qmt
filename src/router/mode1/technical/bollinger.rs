//! 布林线（Bollinger Bands）上/下轨因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;
use tokio::sync::broadcast::Receiver;

use crate::{
    math::{SMA, WindowStats},
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{
        Base, validate_period,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
    },
    toolbox::VJson,
};

/// 布林带轨道：上轨 / 下轨。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum Band {
    /// 上轨：MA + 2 * STD
    Upper,
    /// 下轨：MA - 2 * STD
    Lower,
}

impl Band {
    fn label(self) -> &'static str {
        match self {
            Self::Upper => "上轨",
            Self::Lower => "下轨",
        }
    }

    fn apply(self, mean: f64, std: f64) -> f64 {
        match self {
            Self::Upper => mean + 2.0 * std,
            Self::Lower => mean - 2.0 * std,
        }
    }
}

/// 注册 20 日布林带上/下轨因子。
pub async fn router() -> Router {
    for band in [Band::Upper, Band::Lower] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, 20, band))).await;
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(bollinger)
            .push(Router::with_path("detail").post(bollinger_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 布林带周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
    /// 轨道类型。
    pub band: Band,
}

impl Core {
    fn new(period: usize, band: Band) -> Self {
        Self {
            period: UntArg::new("布林带周期", period),
            band,
        }
    }
}

/// 布林线因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize, band: Band) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period, band),
        }
    }

    fn register(filter: &Filter, period: usize, band: Band) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period, band);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || bollinger_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 布林线因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按布林带轨道值进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_bollinger",
    responses(
        (status_code = 200, description = "布林线因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn bollinger(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || bollinger_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行布林线因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。预热 = `core.period` 个交易日：从 `date` 前 `period` 个交易日
/// 开始喂均线与标准差窗口，保证目标日的轨道值与主分析口径一致。
#[endpoint]
pub async fn bollinger_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || bollinger_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn bollinger_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let band = args.core.band;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("布林线{}{}因子", band.label(), period),
        format!("BOLL:={}(MA(CLOSE,N)+/-2*STD(CLOSE,N)); N:={period}", band.label()),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut mean_store = vec![SMA::new(period); df.list.len()];
    let mut std_store = vec![WindowStats::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, (mean_store, std_store)) in df.list.iter().zip(mean_store.iter_mut().zip(std_store.iter_mut())) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(mean) = mean_store.next(curr.close)
                && let Some(std) = std_store.std(curr.close)
            {
                items.push(Mode1Temp {
                    factor: band.apply(mean, std),
                    profit,
                });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = `period` 个交易日，推进均线与标准差窗口后仅收集目标日当天的轨道值。
fn bollinger_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let band = args.req.core.band;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    // mean 与 std 经 `&&` 串行喂入（std 在 mean 预热后才开始），预热需 2×period。
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period * 2));
    let mut mean_store = vec![SMA::new(period); df.list.len()];
    let mut std_store = vec![WindowStats::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, (mean_store, std_store)) in df.list.iter().zip(mean_store.iter_mut().zip(std_store.iter_mut())) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(mean) = mean_store.next(curr.close)
                && let Some(std) = std_store.std(curr.close)
            {
                if is_target {
                    rows.push(DetailRow::new(&item.metadata, curr, finance, band.apply(mean, std), profit));
                }
            }
        }
    }
    day_value(date, count, rows)
}
