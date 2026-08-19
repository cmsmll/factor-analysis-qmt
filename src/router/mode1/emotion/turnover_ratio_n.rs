//! 换手率比因子：N 日平均换手率与 120 日平均换手率之比。

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

/// 注册 5/10/20 日换手率比因子。
pub async fn router() -> Router {
    for period in [5, 10, 20] {
        MODE1.register(Arc::new(move |filter| Req::register(filter, period))).await;
    }
    Router::new().push(Router::with_path(Req::id()).post(turnover_ratio_n))
}

#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Core {
    /// 短周期，单位为交易日。
    #[validate(custom(function = "validate_period"))]
    pub period: UntArg,
}

impl Core {
    fn new(period: usize) -> Self {
        Self {
            period: UntArg::new("短周期", period),
        }
    }
}

/// 换手率比因子分析请求。
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
        let recv = MODE1.cache.get_or_run(key, move || turnover_ratio_n_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}

/// 按 N 日与 120 日平均换手率之比进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_turnover_ratio_n",
    responses(
        (status_code = 200, description = "换手率比因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn turnover_ratio_n(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || turnover_ratio_n_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn turnover_ratio_n_run(args: Req) -> Box<RawValue> {
    let period = args.core.period.value;
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        format!("{period}日平均换手率与120日平均换手率之比"),
        format!("FACTOR:=MA(TURNOVER,{period})/MA(TURNOVER,120)"),
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    let mut short = vec![SMA::new(period); df.list.len()];
    let mut long = vec![SMA::new(120); df.list.len()];

    for index in df.index_iter() {
        for (item, (short, long)) in df.list.iter().zip(short.iter_mut().zip(long.iter_mut())) {
            if let Some((curr, profit)) = item.data(&index)
                && curr.filter_st(args.base.filter_st)
                && let Some(short_avg) = short.next(curr.turnover)
                && let Some(long_avg) = long.next(curr.turnover)
            {
                items.push(Mode1Temp {
                    factor: dev(short_avg, long_avg),
                    profit,
                });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}
