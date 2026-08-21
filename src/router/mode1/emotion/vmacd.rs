//! 成交量指数平滑异同移动平均因子（VMACD）。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::{
    math::MACD,
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::Base,
    toolbox::VJson,
};

/// VMACD 输出：diff / dea。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum VmacdKind {
    /// DIF（快慢 EMA 差）
    Diff,
    /// DEA（DIF 的 EMA）
    Dea,
}

impl VmacdKind {
    fn label(self) -> &'static str {
        match self {
            Self::Diff => "diff",
            Self::Dea => "dea",
        }
    }

    fn apply(self, macd: &mut MACD, volume: f64) -> Option<f64> {
        match self {
            Self::Diff => macd.dif(volume),
            Self::Dea => macd.dea(volume),
        }
    }
}

/// 注册 VMACD diff / dea 因子。
pub async fn router() -> Router {
    for kind in [VmacdKind::Diff, VmacdKind::Dea] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, kind))).await;
    }
    Router::new().push(Router::with_path(Req::id()).post(vmacd))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 输出类型。
    pub kind: VmacdKind,
}

impl Core {
    fn new(kind: VmacdKind) -> Self {
        Self { kind }
    }
}

/// VMACD 因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(kind: VmacdKind) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(kind),
        }
    }

    fn register(filter: &Filter, kind: VmacdKind) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(kind);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || vmacd_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 按 VMACD diff/dea 值进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_vmacd",
    responses(
        (status_code = 200, description = "VMACD 因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn vmacd(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || vmacd_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn vmacd_run(args: Req) -> Box<RawValue> {
    let kind = args.core.kind;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("VMACD 中间变量{}", kind.label()),
        "DIF:=EMA(VOLUME,12)-EMA(VOLUME,26); DEA:=EMA(DIF,9)",
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![MACD::new(12, 26, 9); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(factor) = kind.apply(store, curr.volume)
            {
                items.push(Mode1Temp { factor, profit });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}
