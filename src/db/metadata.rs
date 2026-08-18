use std::{collections::{HashMap, HashSet}, fs, io, path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

/// 股票元数据。
///
/// 新数据源（`data/metadata.json`）的字段与旧结构存在映射差异：
/// - 数据源字段 `listed_date` → 本结构 `listing_date`（反序列化别名）
/// - 数据源 `code` 带交易所后缀（如 `000001.SZ`），本结构 `code` 剥掉后缀，
///   以匹配成分股索引中的裸代码。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// 交易所（深交所/上交所/北交所）。
    pub exchange: String,
    /// 名称。
    pub name: Arc<str>,
    /// 代码（裸代码，不带交易所后缀）。
    pub code: Arc<str>,
    /// 上市时间。
    #[serde(alias = "listed_date")]
    pub listing_date: String,
    /// 所属板块与指数分类（行业成分股 + 指数成分股合并去重）。
    #[serde(default)]
    pub members: HashSet<String>,
}

/// 从 `data/metadata.json` 加载全部合约元数据，按代码排序。
///
/// JSON 格式为「带后缀代码 -> 元数据对象」，加载时剥离代码后缀。
pub fn load_metadata(path: &Path) -> io::Result<Vec<Metadata>> {
    let content = fs::read(path)?;
    let entries: HashMap<String, Metadata> = serde_json::from_slice(&content).map_err(io::Error::other)?;
    let mut metadata = entries
        .into_values()
        .map(|entry| Metadata {
            exchange: entry.exchange,
            name: entry.name,
            code: Arc::from(normalize_code(&entry.code)),
            listing_date: entry.listing_date,
            members: entry.members,
        })
        .collect::<Vec<_>>();
    metadata.sort_by(|left, right| left.code.cmp(&right.code));
    Ok(metadata)
}

/// 剥离股票代码的交易所后缀，返回裸代码。
pub fn normalize_code(code: &str) -> &str {
    code.strip_suffix(".SH")
        .or_else(|| code.strip_suffix(".SZ"))
        .or_else(|| code.strip_suffix(".BJ"))
        .unwrap_or(code)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    // 测试从新数据源 metadata.json 加载元数据并剥离代码后缀。
    #[test]
    fn load_metadata_strips_code_suffix() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("metadata.json");
        fs::write(
            &path,
            r#"{"000001.SZ":{"name":"平安银行","code":"000001.SZ","exchange":"深交所","listed_date":"1991-04-03"}}"#,
        )
        .unwrap();

        let metadata = load_metadata(&path).unwrap();

        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].code.as_ref(), "000001");
        assert_eq!(metadata[0].name.as_ref(), "平安银行");
        assert_eq!(metadata[0].exchange, "深交所");
        assert_eq!(metadata[0].listing_date, "1991-04-03");
    }

    // 测试 metadata.json 缺失时返回 io 错误。
    #[test]
    fn load_metadata_missing_file_errors() {
        let directory = tempdir().unwrap();
        let error = load_metadata(&directory.path().join("missing.json")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
