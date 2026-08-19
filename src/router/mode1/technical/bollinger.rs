//! 布林线（Bollinger Bands）上/下轨因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::{
    math::{SMA, WindowStats},
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{Base, validate_period},
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
    Router::new().push(Router::with_path(Req::id()).post(bollinger))
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
