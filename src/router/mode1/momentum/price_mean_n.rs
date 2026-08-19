//! 股价与过去 N 日均值之比因子。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::{
    math::{SMA, dev},
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{Base, validate_period},
    toolbox::VJson,
};

/// 注册 21/63/250 日股价均值比因子。
pub async fn router() -> Router {
    for period in [21, 63, 250] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, period))).await;
    }
    Router::new().push(Router::with_path(Req::id()).post(price_mean_n))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 均值窗口，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
}

impl Core {
    fn new(period: usize) -> Self {
        Self {
            period: UntArg::new("均值窗口", period),
        }
    }
}

/// 股价均值比因子分析请求。
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
        let recv = MODE1.cache.get_or_run(key, move || price_mean_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 按当日收盘价与过去 N 日均值之比减一进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_price_mean_n",
    responses(
        (status_code = 200, description = "股价均值比因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn price_mean_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || price_mean_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn price_mean_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("股价均值比因子{period}日"),
        format!("FACTOR:=CLOSE/MA(CLOSE,N)-1; N:={period}"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut store = vec![SMA::new(period); df.list.len()];

    for index in df.index_iter() {
        for (item, store) in df.list.iter().zip(store.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(avg) = store.next(curr.close)
            {
                items.push(Mode1Temp {
                    factor: dev(curr.close, avg) - 1.0,
                    profit,
                });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}
