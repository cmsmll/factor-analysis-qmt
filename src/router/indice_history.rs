//! 指数历史收益数据接口。

use std::{fs, io, path::PathBuf};

use salvo::Writer;
use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use time::Date;

use crate::{CONFIG, prelude::*, reject, resolve, resp::Res, toolbox::VJson, toolbox::serde::date_format};

/// 指数历史收益记录。
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndicePoint {
    /// 日期
    #[serde(with = "date_format")]
    pub datetime: Date,
    /// 日收益率（等比后复权）
    pub profit: f64,
}

/// 指数历史收益请求。
#[derive(Debug, Deserialize, ToSchema, validator::Validate)]
pub struct HistoryReq {
    /// 指数名称（对应 data/indice/{名称}.json）
    #[validate(length(min = 1, message = "指数名称不能为空"))]
    pub name: String,
}

/// 按指数名称获取历史收益数据。
///
/// 读取 `data/indice/{名称}.json`（[{datetime, profit}, ...]），返回全部历史收益。
#[endpoint(
    tags("基础数据"),
    operation_id = "get_indice_history",
    responses(
        (status_code = 200, description = "指数历史收益数据", body = Res<Vec<IndicePoint>>),
        (status_code = 400, description = "指数数据不存在或读取失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
    )
)]
pub fn indice_history(args: VJson<HistoryReq>) -> Resp<Vec<IndicePoint>> {
    let path = indice_file(&args.0.name);
    match load_indice_history(&path) {
        Ok(points) => resolve!(points => 200, "ok"),
        Err(_) => reject!(400, format!("指数 {} 历史数据不存在或读取失败", args.0.name)),
    }
}

/// 指数历史文件路径（按名称，防目录穿越）。
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
