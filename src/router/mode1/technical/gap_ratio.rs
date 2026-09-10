//! 跳空比例因子接口:今日开盘价 / 昨日收盘价 - 1。

use std::sync::Arc;

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;
use tokio::sync::broadcast::Receiver;

use crate::{
    prelude::*,
    reject, resolve,
    resp::Resp,
    router::mode1::{
        Base,
        manager::{day_value, detail_filter, resolve_detail_date, DetailRow},
    },
    toolbox::VJson,
};

/// 注册跳空比例因子接口，并准备默认请求模板和默认结果缓存。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}`
///
/// 初始化路由时会把默认 [`Req`] 写入模式一接口列表，并预先计算默认参数结果。
/// `factor_id` 为 [`Req::id`] 生成的动态值，客户端应通过
/// `POST /api/mode1/list` 获取。
pub async fn router() -> Router {
    MODE1.register(Arc::new(Req::register)).await;
    Router::with_path(Req::id())
        .post(gap_ratio)
        .push(Router::with_path("detail").post(gap_ratio_detail))
}

/// 跳空比例因子分析请求。
///
/// 客户端通常先从 `POST /api/mode1/list` 取得默认结构，再按需修改参数。
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
        let recv = MODE1.cache.get_or_run(key, move || gap_ratio_run(req));
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

/// 跳空比例因子单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。
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

/// 执行跳空比例因子的分位分析。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}`
///
/// 请求头必须包含 `Content-Type: application/json`。请求体使用 [`Req`]，
/// 其中 `base` 包含动态接口 ID、分位数量和股票池筛选条件。
///
/// # Analysis
///
/// 每个交易日使用 `当日开盘价 / 昨日收盘价 - 1` 计算跳空比例，
/// 按比例从低到高排序并切分为 `base.count` 个分位。昨日收盘价为 0 或
/// 当日为其上市首个交易日（无前一日行情）时不参与当日计算。
///
/// # Response
///
/// 成功时返回 `200`，`data` 为 [`Mode1Data`]。JSON 解析失败或请求头错误
/// 由提取器返回 `415`；后台分析任务失败时返回 `400` 和 `"获取数据失败"`。
#[endpoint(
    tags("模式一"),
    operation_id = "analyze_gap_ratio",
    responses(
        (status_code = 200, description = "跳空比例因子分析结果", body = Res<Mode1Data>),
        (status_code = 400, description = "分析任务失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn gap_ratio(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.cache.get_or_run(key, move || gap_ratio_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 根据请求参数计算跳空比例分位数据。
///
/// 需要前一根 K 线的收盘价作为昨日收盘价：筛选区间起点当天因缺少
/// 昨收而不参与计算（与其它带预热因子一致，warmup = 1）。
fn gap_ratio_run(args: Req) -> Box<RawValue> {
    let df = DF.filter(&args.base.filter);
    let mut result = Mode1Data::new(
        args.hashcode(),
        "跳空比例因子",
        "跳空比例:=(OPEN/REF(CLOSE,1))-1",
        super::LABEL,
        args.base.count,
    );
    let mut items = Vec::with_capacity(df.list.len());
    // 每只合约记录上一交易日收盘价
    let mut prev_close = vec![None; df.list.len()];

    for index in df.index_iter() {
        for (item, prev) in df.list.iter().zip(prev_close.iter_mut()) {
            if let Some((curr, profit)) = item.data(&index) {
                if let Some(yesterday) = *prev {
                    if yesterday > 0.0 && curr.filter_st(args.base.filter_st) {
                        items.push(Mode1Temp {
                            factor: curr.open / yesterday - 1.0,
                            profit,
                        });
                    }
                }
                *prev = Some(curr.close);
            }
        }
        result.push(index.datetime, &mut items);
        unsafe { items.set_len(0) }
    }

    result.raw_value()
}

/// 执行跳空比例因子目标日单日分位明细查询。
///
/// # Route
///
/// `POST /api/mode1/{factor_id}/detail`
///
/// 请求体为 [`DetailReq`]：在 [`Req`] 基础上可带目标日期 `date`（`YYYY-MM-DD`），
/// 缺省取筛选区间末交易日。跳空比例需要前一日收盘价，warmup = 1：
/// 从目标日的前一交易日开始加载，仅收集目标日当天的分位明细行。
#[endpoint]
pub async fn gap_ratio_detail(args: VJson<DetailReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE1.details.get_or_run(key, move || gap_ratio_detail_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

/// 计算目标日单日分位明细：warmup = 1，需前一交易日收盘价。
fn gap_ratio_detail_run(args: DetailReq) -> Box<RawValue> {
    let count = args.req.base.count;
    let date = resolve_detail_date(args.date, &args.req.base.filter);
    let df = DF.filter(&detail_filter(&args.req.base.filter, date, 1));
    let mut prev_close = vec![None; df.list.len()];
    let mut rows: Vec<DetailRow> = Vec::with_capacity(df.list.len());

    for index in df.index_iter() {
        let is_target = index.datetime == date;
        for (item, prev) in df.list.iter().zip(prev_close.iter_mut()) {
            if let Some((curr, profit, finance)) = item.data_and_finance(&index) {
                if is_target
                    && let Some(yesterday) = *prev
                    && yesterday > 0.0
                    && curr.filter_st(args.req.base.filter_st)
                {
                    rows.push(DetailRow::new(
                        &item.metadata,
                        curr,
                        finance,
                        curr.open / yesterday - 1.0,
                        profit,
                    ));
                }
                *prev = Some(curr.close);
            }
        }
    }
    day_value(date, count, rows)
}
