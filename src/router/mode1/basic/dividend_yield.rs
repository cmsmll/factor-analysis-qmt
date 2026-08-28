//! 股息率因子接口。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::{prelude::*, reject, resolve, resp::Resp, router::mode1::Base, toolbox::VJson};

/// 注册股息率因子接口，并加入模式一因子列表。
pub async fn router() -> Router {
    MODE1.register(Arc::new(Req::register)).await;
    Router::with_path(Req::id()).post(dividend_yield)
}

/// 股息率因子分析请求。
#[derive(Debug, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    #[validate(nested)]
    base: Base,
}

impl Req {
    fn register(filter: &Filter) -> (Arc<RawValue>, Receiver<Arc<RawValue>>) {
        let mut req = Self::default();
        req.base.filter = filter.clone();
        let value = Arc::from(req.raw_value());
        let key = req.hashcode();
        let recv = MODE1.cache.get_or_run(key, move || dividend_yield_run(req));
        (value, recv)
    }
}

impl ArgsHandle for Req {}
impl Default for Req {
    fn default() -> Self {
        Self {
            base: Base {
                id: Self::id(),
                count: 5,
                filter: Filter::from_config(&CONFIG),
            },
        }
    }
}

/// 按当日股息率进行分位分析。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_dividend_yield",
    responses(
        (status_code = 200, description = "股息率因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn dividend_yield(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || dividend_yield_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn dividend_yield_run(args: Req) -> Box<RawValue> {
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        "股息率因子",
        "按当日股息率从低到高分位",
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        for item in &df.list {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index)
                && curr.filter_st(args.base.filter_st)
            {
                items.push(Mode1Temp {
                    factor: finance.dividend_yield,
                    profit,
                });
            }
        }
        result.push(index.datetime, &mut items);
        items.clear();
    }

    result.raw_value()
}
