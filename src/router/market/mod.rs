//! 行情预览（market）接口：统一快照列表 + 单股全量 K 线。
//!
//! 数据只读全局内存 `DF`（DataFrameDb 已加载 data/market 全部合约），不重读文件、不落缓存。
//!
//! - `GET /api/market/list`：统一快照交易日 = `DF.end`（末交易日），当日无行（停牌/滞后/未上市）的合约剔除，当日全量一次返回；
//!   字段已归一为前端展示口径：`change_percent` 为小数、`volume` 单位为股、`turnover_rate` 为换手率小数。
//! - `GET /api/market/{code}/kline`：按裸代码定位合约返回全量日 K，结构镜像参考工程 data.json
//!   `{ nam, code, market_data: [{datetime, change_percent, open, close, high, low, volume, turnover, turnover_rate}] }`
//!   （`turnover` = 成交额元、`volume` = 股、`change_percent`/`turnover_rate` 为小数），供前端组件原样复用。

use std::sync::Arc;

use salvo::{Request, Router};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use time::{Date, format_description::well_known::Iso8601};

use crate::{DF, reject, rejectf, resolve, resp::{Res, Resp}};

/// 行情快照行（列表接口，字段已归一）。
#[derive(Debug, Serialize, ToSchema)]
pub struct SnapshotRow {
    /// 证券代码（裸码）
    pub code: String,
    /// 证券名称
    pub name: String,
    /// 交易所
    pub exchange: String,
    /// 快照交易日
    pub datetime: String,
    /// 涨跌幅（小数，如 0.0094 = +0.94%）
    pub change_percent: f64,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    /// 成交量（股）
    pub volume: f64,
    /// 成交额（元）
    pub amount: f64,
    /// 换手率（小数，如 0.0041 = 0.41%）
    pub turnover_rate: f64,
    /// 所属行业/指数分类（升序）
    pub tags: Vec<String>,
}

/// 路径参数：裸代码。
#[derive(Debug, Deserialize)]
struct CodeReq {
    code: String,
}

/// 构建 market 路由树。
pub async fn market_router() -> Router {
    Router::with_path("market")
        .push(Router::with_path("list").get(market_list))
        .push(Router::with_path("{code}").push(Router::with_path("kline").get(market_kline)))
}

/// 统一快照交易日全量行情列表。
#[endpoint(
    tags("基础数据"),
    operation_id = "market_snapshot_list",
    responses(
        (status_code = 200, description = "行情快照列表", body = Res<Vec<SnapshotRow>>),
    )
)]
/// 全量行情快照列表。
///
/// 快照日取 `date` 参数(需为交易日,否则返回空);缺省为末交易日 `DF.end`。
/// 当日无行(停牌/滞后/未上市)的合约剔除。
pub async fn market_list(req: &mut Request) -> Resp<Vec<SnapshotRow>> {
    let requested: Option<String> = req.query("date");
    let date = match requested {
        Some(text) => match Date::parse(&text, &Iso8601::DATE) {
            Ok(value) => value,
            Err(_) => return reject!(400, "日期格式应为 YYYY-MM-DD"),
        },
        None => DF.end,
    };
    let mut rows = Vec::with_capacity(DF.list.len());
    for contract in &DF.list {
        let Some(&position) = contract.table.get(&date) else { continue };
        let market = &contract.bar[position].market;
        let mut tags: Vec<String> = contract.metadata.members.iter().cloned().collect();
        tags.sort_unstable();
        rows.push(SnapshotRow {
            code: contract.metadata.code.to_string(),
            name: contract.metadata.name.to_string(),
            exchange: contract.metadata.exchange.clone(),
            datetime: market.datetime.to_string(),
            change_percent: market.change_percent / 100.0,
            open: market.open,
            close: market.close,
            high: market.high,
            low: market.low,
            volume: market.volume * 100.0,
            amount: market.amount,
            turnover_rate: market.turnover,
            tags,
        });
    }
    resolve!(rows => 200, "ok")
}

/// 单股全量日 K 线。
#[endpoint(
    tags("基础数据"),
    operation_id = "market_kline",
    responses(
        (status_code = 200, description = "单股全量日K线（Contract 全量，profit/table 跳过）"),
        (status_code = 404, description = "股票不存在", body = Res<()>),
    )
)]
pub async fn market_kline(req: &mut Request) -> Resp<Arc<RawValue>> {
    let code = match req.parse_params::<CodeReq>() {
        Ok(args) => args.code,
        Err(_) => return reject!(400, "路径参数错误"),
    };
    let Some(contract) = DF.list.iter().find(|item| item.metadata.code.as_ref() == code) else {
        return rejectf!(404, "股票不存在: {code}");
    };

    // 直接返回全量 Contract（metadata + 全量 bar；profit/table 序列化已跳过）
    let json = serde_json::to_string(contract.as_ref()).unwrap();
    resolve!(Arc::from(RawValue::from_string(json).unwrap()) => 200, "ok")
}
