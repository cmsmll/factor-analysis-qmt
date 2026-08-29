//! 模式二：因子选股接口（排序 → 过滤 → 截取前 N）。

use std::{path::Path, sync::Arc};

use salvo_oapi::{ToSchema, endpoint};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use time::{Date, macros::date};

use crate::{
    DF, MODE2,
    args::Filter as PoolFilter,
    cache::Cache,
    prelude::*,
    toolbox::VJson,
};

use operator::{Direction, Field, Filter as OpFilter};

pub mod engine;
pub mod operator;

/// 选股算子链中的一段：对前一段输出执行「排序 → 过滤 → 截取前 N」。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Stage {
    /// 因子字段
    pub field: Field,
    /// 排序方向
    pub direction: Direction,
    /// 过滤条件（作用于排序后的因子字段）
    pub filter: OpFilter,
    /// 选股数量（前 N 名）
    #[validate(range(min = 1, message = "选股数量必须大于等于 1"))]
    pub select: usize,
}

/// 模式二选股请求参数。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct Req {
    /// 选股算子链（顺序执行，至少 1 段；前一段输出作为后一段输入）
    #[validate(length(min = 1, message = "算子链至少 1 段"))]
    pub stages: Vec<Stage>,
    /// 收益模式（1..=4，对应 `Bar.profit` 的 p1..p4；1=次日收盘收益）
    #[validate(range(min = 1, max = 4, message = "收益模式必须在 1..=4"))]
    pub profit_mode: u8,
    /// 股票池与日期筛选
    #[validate(nested)]
    pub base: PoolFilter,
}

impl Default for Req {
    fn default() -> Self {
        // 微盘股预设：市值最小 400 只 → 其中收盘价最低 80 只。
        Self {
            stages: vec![
                Stage {
                    field: Field::TotalMarket,
                    direction: Direction::Asc,
                    filter: OpFilter::None,
                    select: 400,
                },
                Stage {
                    field: Field::Close,
                    direction: Direction::Asc,
                    filter: OpFilter::None,
                    select: 80,
                },
            ],
            profit_mode: 1,
            base: PoolFilter::new(date!(2025-01-01), date!(2025-12-31)),
        }
    }
}

impl ArgsHandle for Req {}
impl ArgsHandle for SelectReq {}

/// 单日选股请求：`Req` + 可选日期（缺省取 base 区间最后一天）。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct SelectReq {
    #[serde(flatten)]
    #[validate(nested)]
    pub req: Req,
    /// 选股日期（缺省取 base 区间最后一天）
    #[serde(default, with = "crate::toolbox::serde::date_format::opt")]
    pub date: Option<Date>,
}

/// mode2 管理器：持有缓存。
pub struct Mode2Manager {
    cache: Cache,
}

impl Mode2Manager {
    pub fn new(base: &Path) -> Self {
        Self {
            cache: Cache::sub(base, "mode2").expect("创建 mode2 缓存目录失败"),
        }
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }
}

/// 构建模式二的路由树。
pub async fn mode2_router() -> Router {
    Router::with_path("mode2")
        .push(Router::with_path("select").post(select))
        .push(Router::with_path("history").post(history))
}

/// 单日选股名单。
#[endpoint(
    tags("模式二"),
    operation_id = "mode2_select",
    responses(
        (status_code = 200, description = "选股名单", body = Res<Vec<engine::StockItem>>),
        (status_code = 400, description = "获取数据失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn select(args: VJson<SelectReq>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE2.cache().get_or_run(key, move || select_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn select_run(req: SelectReq) -> Box<RawValue> {
    let frame = DF.filter(&req.req.base);
    let date = req
        .date
        .unwrap_or_else(|| frame.index.last().copied().unwrap_or(req.req.base.end));
    let items = engine::select_at(&frame, &req.req, date);
    let s = serde_json::to_string(&items).unwrap();
    RawValue::from_string(s).unwrap()
}

/// 区间回测：逐日选股、组合与基准净值、调仓换手率与统计。
#[endpoint(
    tags("模式二"),
    operation_id = "mode2_history",
    responses(
        (status_code = 200, description = "区间回测结果", body = Res<engine::Mode2History>),
        (status_code = 400, description = "获取数据失败", body = Res<()>),
        (status_code = 422, description = "参数校验失败", body = Res<()>),
        (status_code = 415, description = "Content-Type 或 JSON 请求体错误", body = Res<()>),
    )
)]
pub async fn history(args: VJson<Req>) -> Resp<Arc<RawValue>> {
    let key = args.0.hashcode();
    match MODE2.cache().get_or_run(key, move || history_run(args.0)).recv().await {
        Ok(res) => resolve!(res => 200, "ok"),
        Err(_) => reject!(400, "获取数据失败"),
    }
}

fn history_run(req: Req) -> Box<RawValue> {
    let frame = DF.filter(&req.base);
    let result = engine::history(&frame, &req);
    let s = serde_json::to_string(&result).unwrap();
    RawValue::from_string(s).unwrap()
}
