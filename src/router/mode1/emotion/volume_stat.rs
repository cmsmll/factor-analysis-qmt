//! 量能统计类因子：量变动速率、成交量震荡、成交量/成交金额标准差、tvma6、VMACD。

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

/// 量能统计因子类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum VolStatKind {
    /// 量变动速率（6 日）
    Roc6,
    /// 量变动速率（12 日）
    Roc12,
    /// 成交量震荡
    Osc,
    /// 成交量标准差（10 日）
    StdVol10,
    /// 成交量标准差（20 日）
    StdVol20,
    /// 成交金额标准差（6 日）
    StdAmt6,
    /// 成交金额标准差（20 日）
    StdAmt20,
    /// tvma6（量价加权均线）
    Tvma6,
}

impl VolStatKind {
    fn label(self) -> &'static str {
        match self {
            Self::Roc6 => "6 日量变动速率",
            Self::Roc12 => "12 日量变动速率",
            Self::Osc => "成交量震荡",
            Self::StdVol10 => "10 日成交量标准差",
            Self::StdVol20 => "20 日成交量标准差",
            Self::StdAmt6 => "6 日成交金额标准差",
            Self::StdAmt20 => "20 日成交金额标准差",
            Self::Tvma6 => "tvma6",
        }
    }

    fn window(self) -> usize {
        match self {
            Self::Roc6 | Self::StdAmt6 | Self::Tvma6 => 6,
            Self::Roc12 => 12,
            Self::StdVol10 => 10,
            Self::Osc | Self::StdVol20 | Self::StdAmt20 => 20,
        }
    }
}

/// 注册全部量能统计因子。
pub async fn router() -> Router {
    for kind in [
        VolStatKind::Roc6,
        VolStatKind::Roc12,
        VolStatKind::Osc,
        VolStatKind::StdVol10,
        VolStatKind::StdVol20,
        VolStatKind::StdAmt6,
        VolStatKind::StdAmt20,
        VolStatKind::Tvma6,
    ] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, kind.window(), kind))).await;
    }
    Router::new().push(
        Router::with_path(Req::id())
            .post(vol_stat)
            .push(Router::with_path("detail").post(vol_stat_detail)),
    )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 量能统计周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
    /// 指标类型。
    pub kind: VolStatKind,
}

impl Core {
    fn new(period: usize, kind: VolStatKind) -> Self {
        Self {
            period: UntArg::new("统计周期", period),
            kind,
        }
    }
}

/// 量能统计因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(period: usize, kind: VolStatKind) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(period, kind),
        }
    }

    fn register(filter: &Filter, period: usize, kind: VolStatKind) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(period, kind);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || vol_stat_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 量能统计因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 按量能统计指标进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_vol_stat",
    responses(
        (status_code = 200, description = "量能统计因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn vol_stat(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || vol_stat_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 执行量能统计因子目标日单日分位明细查询。
///
/// 预热按类型区分：Roc6/Roc12 仅依赖前一交易日的成交量（warmup = 2）；
/// Osc/StdVol10/StdVol20/StdAmt6/StdAmt20/Tvma6 依赖 `kind.window()` 长度的
/// WindowStats（warmup = `kind.window()`），与主分析口径一致。
#[endpoint]
pub async fn vol_stat_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || vol_stat_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn vol_stat_run(args: Req) -> Box<RawValue> {
    let kind = args.core.kind;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        kind.label(),
        format!("{}; N:={}", kind.label(), args.core.period.value),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut stats = vec![WindowStats::new(kind.window()); df.list.len()];
    let mut prev_vol = vec![0.0f64; df.list.len()];

    for index in df.index_iter() {
        for (item, (stats, prev_vol)) in df.list.iter().zip(stats.iter_mut().zip(prev_vol.iter_mut())) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
            {
                let factor = match kind {
                    VolStatKind::Roc6 | VolStatKind::Roc12 => {
                        let prev = *prev_vol;
                        *prev_vol = curr.volume;
                        (prev != 0.0).then_some(dev(curr.volume - prev, prev) * 100.0)
                    }
                    VolStatKind::Osc => stats.mean(curr.volume).map(|mean| dev(curr.volume - mean, mean) * 100.0),
                    VolStatKind::StdVol10 | VolStatKind::StdVol20 => stats.std(curr.volume),
                    VolStatKind::StdAmt6 | VolStatKind::StdAmt20 => stats.std(curr.amount),
                    VolStatKind::Tvma6 => stats.mean(curr.volume * curr.close),
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

/// 计算目标日单日分位明细：Roc6/Roc12 预热 2 个交易日（仅需前一日成交量），其余预热
/// `kind.window()` 个交易日推进 WindowStats；仅收集目标日当天分位行。
fn vol_stat_detail_run(args: DetailReq) -> Box<RawValue> {
    let kind = args.req.core.kind;
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let warmup = match kind {
        VolStatKind::Roc6 | VolStatKind::Roc12 => 2,
        _ => kind.window(),
    };
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, warmup));
    let mut stats = vec![WindowStats::new(kind.window()); df.list.len()];
    let mut prev_vol = vec![0.0f64; df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, (stats, prev_vol)) in df.list.iter().zip(stats.iter_mut().zip(prev_vol.iter_mut())) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.req.base.filter_st)
            {
                let factor = match kind {
                    VolStatKind::Roc6 | VolStatKind::Roc12 => {
                        let prev = *prev_vol;
                        *prev_vol = curr.volume;
                        (prev != 0.0).then_some(dev(curr.volume - prev, prev) * 100.0)
                    }
                    VolStatKind::Osc => stats.mean(curr.volume).map(|mean| dev(curr.volume - mean, mean) * 100.0),
                    VolStatKind::StdVol10 | VolStatKind::StdVol20 => stats.std(curr.volume),
                    VolStatKind::StdAmt6 | VolStatKind::StdAmt20 => stats.std(curr.amount),
                    VolStatKind::Tvma6 => stats.mean(curr.volume * curr.close),
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
