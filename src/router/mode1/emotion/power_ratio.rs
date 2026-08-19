//! 多头力道 / 空头力道因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::{
    math::dev,
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::Base,
    toolbox::VJson,
};

/// 力道类型：多头 / 空头。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum PowerKind {
    /// 多头力道
    Bull,
    /// 空头力道
    Bear,
}

impl PowerKind {
    fn label(self) -> &'static str {
        match self {
            Self::Bull => "多头力道",
            Self::Bear => "空头力道",
        }
    }
}

/// 注册多头/空头力道因子。
pub async fn router() -> Router {
    for kind in [PowerKind::Bull, PowerKind::Bear] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, kind))).await;
    }
    Router::new().push(Router::with_path(Req::id()).post(power_ratio))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 力道类型。
    pub kind: PowerKind,
}

impl Core {
    fn new(kind: PowerKind) -> Self {
        Self { kind }
    }
}

/// 力道因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
    #[validate(nested)]
    core: Core,
}

impl Req {
    fn new(kind: PowerKind) -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
            core: Core::new(kind),
        }
    }

    fn register(filter: &Filter, kind: PowerKind) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::new(kind);
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || power_ratio_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 按当日多头/空头力道进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_power_ratio",
    responses(
        (status_code = 200, description = "力道因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn power_ratio(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || power_ratio_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn power_ratio_run(args: Req) -> Box<RawValue> {
    let kind = args.core.kind;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        kind.label(),
        match kind {
            PowerKind::Bull => "多头力道:=(HIGH-OPEN)/(HIGH-LOW)*VOLUME",
            PowerKind::Bear => "空头力道:=(OPEN-LOW)/(HIGH-LOW)*VOLUME",
        },
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        for item in &df.list {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
            {
                let range = curr.high - curr.low;
                let factor = match kind {
                    PowerKind::Bull => dev(curr.high - curr.open, range) * curr.volume,
                    PowerKind::Bear => dev(curr.open - curr.low, range) * curr.volume,
                };
                items.push(Mode1Temp { factor, profit });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}
