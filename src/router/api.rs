//! 基础数据接口(挂载于 `/api` 下):股票池指数/行业/分析周期/数据日期区间。
//!
//! - `GET /api/indice`：指数列表；`POST /api/indice`：按名称取指数历史收益
//! - `GET /api/sector`：行业板块列表
//! - `GET /api/period`：分析周期预设
//! - `GET /api/range`：全市场交易日最早/最晚日期；`POST /api/range`：单只股票行情首/末日期

use std::{collections::HashSet, fs, io, path::PathBuf, sync::Arc};

use salvo::{Router, Writer};
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;

use crate::{
    CONFIG, DF,
    config::Period,
    reject, rejectf, res, resolve,
    resp::{Res, Resp},
    toolbox::VJson,
    toolbox::serde::date_format,
};

/// 组装基础数据路由片段(由 router/mod.rs 挂到 `/api` 下组合)。
pub fn api_router() -> Router {
    Router::new()
        .push(Router::with_path("sector").get(sector))
        .push(Router::with_path("period").get(period))
        .push(Router::with_path("range").get(range).post(range_by_code))
        .push(Router::with_path("indice").get(indice).post(indice_history))
}

/// 数据日期区间(YYYY-MM-DD)。
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DataRange {
    /// 最早交易日
    pub min_date: String,
    /// 最晚交易日(末交易日)
    pub max_date: String,
}

/// 获取股票池指数列表。
///
/// 返回成分股 JSON(`data/indice.json`)中全部指数分类的去重集合。集合序列化后的顺序不固定。
#[endpoint(
    tags("基础数据"),
    operation_id = "list_indices",
    responses((status_code = 200, description = "指数列表", body = Res<HashSet<String>>))
)]
fn indice() -> Res<Arc<HashSet<String>>> {
    res!(DF.indice.clone() => 200, "ok")
}

/// 获取股票池行业板块列表。
///
/// 返回成分股 JSON(`data/sector.json`)中全部行业分类的去重集合。
#[endpoint(
    tags("基础数据"),
    operation_id = "list_sectors",
    responses((status_code = 200, description = "行业板块列表", body = Res<HashSet<String>>))
)]
fn sector() -> Res<Arc<HashSet<String>>> {
    res!(DF.sector.clone() => 200, "ok")
}

/// 获取分析周期预设列表。
#[endpoint]
fn period() -> Res<Vec<Period>> {
    res!(CONFIG.period.clone() => 200, "ok")
}

/// 全市场数据日期区间:最早/最晚交易日(日历边界,YYYY-MM-DD)。
#[endpoint(
    tags("基础数据"),
    operation_id = "data_range",
    responses((status_code = 200, description = "数据日期区间", body = Res<DataRange>))
)]
fn range() -> Res<DataRange> {
    res!(
        DataRange {
            min_date: DF.start.to_string(),
            max_date: DF.end.to_string(),
        } => 200,
        "ok"
    )
}

/// 单股区间请求体：裸代码。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, validator::Validate)]
struct CodeRangeReq {
    code: String,
}

/// 查询单只股票自身行情时间范围(该股首/末交易日,YYYY-MM-DD)。
#[endpoint(
    tags("基础数据"),
    operation_id = "stock_data_range",
    responses(
        (status_code = 200, description = "单股数据日期区间", body = Res<DataRange>),
        (status_code = 404, description = "股票不存在或无行情", body = Res<()>),
    )
)]
async fn range_by_code(args: VJson<CodeRangeReq>) -> Resp<DataRange> {
    let Some(contract) = DF.list.iter().find(|item| item.metadata.code.as_ref() == args.code.as_str()) else {
        return rejectf!(404, "股票不存在: {}", args.code);
    };
    let first = match contract.bar.first() {
        Some(bar) => bar,
        None => return rejectf!(404, "股票无行情数据: {}", args.code),
    };
    let last = contract.bar.last().unwrap_or(first);
    resolve!(
        DataRange {
            min_date: first.market.datetime.to_string(),
            max_date: last.market.datetime.to_string(),
        } => 200,
        "ok"
    )
}

/// 指数历史收益记录。
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndicePoint {
    /// 日期
    #[serde(with = "date_format")]
    pub datetime: Date,
    /// 日收益率(等比后复权)
    pub profit: f64,
}

/// 指数历史收益请求。
#[derive(Debug, Deserialize, ToSchema, validator::Validate)]
pub struct HistoryReq {
    /// 指数名称(对应 data/indice/{名称}.json)
    #[validate(length(min = 1, message = "指数名称不能为空"))]
    pub name: String,
}

/// 按指数名称获取历史收益数据。
///
/// 读取 `data/indice/{名称}.json`([{datetime, profit}, ...]),返回全部历史收益。
#[endpoint(
    tags("基础数据"),
    operation_id = "get_indice_history",
    responses(
        (status_code = 200, description = "指数历史收益数据", body = Res<Vec<IndicePoint>>),
        (status_code = 400, description = "指数数据不存在或读取失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
    )
)]
fn indice_history(args: VJson<HistoryReq>) -> Resp<Vec<IndicePoint>> {
    let path = indice_file(&args.0.name);
    match load_indice_history(&path) {
        Ok(points) => resolve!(points => 200, "ok"),
        Err(_) => reject!(400, format!("指数 {} 历史数据不存在或读取失败", args.0.name)),
    }
}

/// 指数历史文件路径(按名称,防目录穿越)。
fn indice_file(name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '·')
        .collect();
    CONFIG.data.indice_dir.join(format!("{safe}.json"))
}

fn load_indice_history(path: &PathBuf) -> io::Result<Vec<IndicePoint>> {
    let content = fs::read(path)?;
    serde_json::from_slice(&content).map_err(io::Error::other)
}
