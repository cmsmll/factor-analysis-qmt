//! 个股收益统计因子：方差、偏度、峰度。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;
use tokio::sync::broadcast::Receiver;

use crate::{
    math::{WindowStats, dev},
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

/// 统计量类型：方差 / 偏度 / 峰度。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum StatKind {
    /// 收益方差
    Variance,
    /// 收益偏度
    Skewness,
    /// 收益峰度（超额峰度）
    Kurtosis,
}

impl StatKind {
    fn label(self) -> &'static str {
        match self {
            Self::Variance => "方差",
            Self::Skewness => "偏度",
            Self::Kurtosis => "峰度",
        }
    }

    fn apply(self, stats: &mut WindowStats, value: f64) -> Option<f64> {
        match self {
            Self::Variance => stats.std(value).map(|std| std * std),
            Self::Skewness => stats.skewness(value),
            Self::Kurtosis => stats.kurtosis(value),
        }
    }
}

/// 注册 20/60/120 日收益方差/偏度/峰度因子。
pub async fn router() -> Router {
    for period in [20, 60, 120] {
        for kind in [StatKind::Variance, StatKind::Skewness, StatKind::Kurtosis] {
            MODE1.register(Arc::new(move |filter| Req::register(filter, period, kind))).await;
        }
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(return_stat_n)
            .push(Router::with_path("detail").post(return_stat_n_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 统计窗口，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
    /// 统计量类型。
    pub kind: StatKind,
}

impl Core {
    fn new(period: usize, kind: StatKind) -> Self {
        Self {
            period: UntArg::new("统计窗口", period),
            kind,
        }
    }
}

/// 收益统计因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize, kind: StatKind) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period, kind),
        }
    }

    fn register(filter: &Filter, period: usize, kind: StatKind) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period, kind);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || return_stat_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 收益统计因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按 N 日收益统计量进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_return_stat_n",
    responses(
        (status_code = 200, description = "收益统计因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn return_stat_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || return_stat_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行收益统计因子目标日单日分位明细查询。
///
/// 预热 = `core.period` 个交易日：从 `date` 前 `period` 个交易日开始喂 WindowStats，
/// 保证目标日的收益统计量与主分析口径一致（WindowStats 只依赖最近 `period` 个交易日）。
#[endpoint]
pub async fn return_stat_n_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || return_stat_n_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn return_stat_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let kind = args.core.kind;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("个股收益{}{}日", kind.label(), period),
        format!("RETURN:=(CLOSE-REF(CLOSE,1))/REF(CLOSE,1); STAT:={}(RETURN,N); N:={period}", kind.label()),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![WindowStats::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = kind.apply(store, dev(curr.close - prev.close, prev.close))
            {
                items.push(Mode1Temp { factor, profit });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}

/// 计算目标日单日分位明细：预热 = `period` 个交易日，从目标日回推逐日推进 WindowStats
/// （每日需先取前一交易日收益，守卫与主分析一致），仅收集目标日的收益统计量。
fn return_stat_n_detail_run(args: DetailReq) -> Box<RawValue> {
    let period = args.req.core.period.value;
    let kind = args.req.core.kind;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, period));
    let mut store = vec![WindowStats::new(period); df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
                && let Some(prev) = item.before(&index, 1)
                && let Some(factor) = kind.apply(store, dev(curr.close - prev.close, prev.close))
            {
                if is_target {
                    rows.push(DetailRow::new(&item.metadata, curr, finance, factor, profit));
                }
            }
        }
    }
    day_value(date, count, rows)
}
