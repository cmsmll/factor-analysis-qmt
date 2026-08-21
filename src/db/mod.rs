pub mod bar_date;
pub mod dataframe;
pub mod metadata;

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs, io,
    path::Path,
    sync::Arc,
};

pub use bar_date::*;
pub use dataframe::*;
pub use metadata::*;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use time::Date;

use crate::config::Config;

/// 内存数据仓库。
pub struct DataFrameDb {
    /// 合约信息，与 bar 使用相同顺序。
    pub metadata: Vec<Arc<Metadata>>,
    /// 每只合约的逐日行情与财务数据，与 metadata 使用相同顺序。
    pub bar: Vec<Arc<Vec<Bar>>>,
    /// 全部行业分类列表。
    pub sector: Arc<HashSet<String>>,
    /// 全部指数分类列表。
    pub indice: Arc<HashSet<String>>,
}

/// 新数据源单行：行情与财务字段合并于同一 JSON 行。
#[derive(Debug, Deserialize)]
struct MarketRow {
    #[serde(with = "crate::toolbox::serde::date_format")]
    datetime: Date,
    #[serde(alias = "change_pct")]
    change_percent: f64,
    open: f64,
    close: f64,
    high: f64,
    low: f64,
    volume: f64,
    amount: f64,
    turnover: f64,
    #[serde(default)]
    dividend_yield: f64,
    is_st: bool,
    #[serde(default)]
    total_market: f64,
}

impl MarketRow {
    fn to_bar(&self) -> Bar {
        Bar {
            market: Market {
                datetime: self.datetime,
                change_percent: self.change_percent,
                open: self.open,
                close: self.close,
                high: self.high,
                low: self.low,
                volume: self.volume,
                amount: self.amount,
                turnover: self.turnover,
                dividend_yield: self.dividend_yield,
                is_st: self.is_st,
            },
            finance: Finance {
                total_market: self.total_market,
            },
        }
    }
}

impl DataFrameDb {
    /// 从配置中的数据路径加载新 JSON 数据源。
    pub fn from_config(config: &Config) -> io::Result<Self> {
        let data = &config.data;
        let mut metadata = load_metadata(&data.metadata)?;

        // 行业/指数成分股：group keys 收集全列表，并按代码反转回填 members。
        let sector_groups = load_groups(&data.sector_json)?;
        let indice_groups = load_groups(&data.indice_json)?;
        let sector: Arc<HashSet<String>> = Arc::new(sector_groups.keys().cloned().collect());
        let indice: Arc<HashSet<String>> = Arc::new(indice_groups.keys().cloned().collect());

        // 代码(裸) -> 合并后的行业+指数分类集合。
        let mut members_by_code: FxHashMap<&str, HashSet<String>> = FxHashMap::default();
        for (group, codes) in sector_groups.iter().chain(indice_groups.iter()) {
            for code in codes {
                members_by_code.entry(normalize_code(code)).or_default().insert(group.clone());
            }
        }
        for meta in &mut metadata {
            if let Some(members) = members_by_code.get(meta.code.as_ref()) {
                meta.members = members.clone();
            }
        }

        // 代码 -> 元数据索引，代码已剥后缀。
        let metadata_by_code = metadata
            .iter()
            .map(|meta| (meta.code.clone(), Arc::new(meta.clone())))
            .collect::<FxHashMap<_, _>>();

        // 遍历行情 JSON 目录，文件名 stem 为带后缀代码。
        let mut contracts: Vec<Arc<Metadata>> = Vec::new();
        let mut bar: Vec<Arc<Vec<Bar>>> = Vec::new();

        let mut files = fs::read_dir(&data.market)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?;
        files.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "json"));
        files.sort();

        for path in files {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(meta) = metadata_by_code.get(normalize_code(stem)) else {
                eprintln!("跳过无元数据的行情文件: {}", path.display());
                continue;
            };
            let rows = load_market_rows(&path)?;
            if rows.is_empty() {
                continue;
            }
            let bars = rows.iter().map(MarketRow::to_bar).collect::<Vec<_>>();
            contracts.push(meta.clone());
            bar.push(Arc::new(bars));
        }

        Ok(Self {
            metadata: contracts,
            bar,
            sector,
            indice,
        })
    }

    /// 查询全部行情，并为每条行情关联同期的财务数据。
    pub fn query_all(&self) -> io::Result<DataFrame> {
        self.build(None)
    }

    /// 查询指定日期范围内的行情（含边界），并为每条行情关联同期财务数据。
    pub fn query(&self, start: Date, end: Date) -> io::Result<DataFrame> {
        self.build(Some((start, end)))
    }

    fn build(&self, range: Option<(Date, Date)>) -> io::Result<DataFrame> {
        let mut list = Vec::new();
        let mut index_table = BTreeSet::new();

        for (meta, bars) in self.metadata.iter().zip(self.bar.iter()) {
            // 范围过滤。
            let Some(bars) = filter_range(bars, range) else {
                continue;
            };

            // 计算前向收益（需要至少 3 条数据）。
            let profit = bars
                .windows(3)
                .map(|w| {
                    let curr = &w[0].market;
                    let next1 = &w[1].market;
                    let next2 = &w[2].market;
                    [
                        (next1.close - curr.close) / curr.close,
                        (next1.close - next1.open) / next1.open,
                        (next2.open - next1.open) / next1.open,
                        (next2.close - next1.open) / next1.open,
                        curr.turnover,
                    ]
                })
                .collect();

            for bar in &bars {
                index_table.insert(bar.market.datetime);
            }

            let table = bars
                .iter()
                .enumerate()
                .map(|(i, bar)| (bar.market.datetime, i))
                .collect::<FxHashMap<_, _>>();
            let start = bars[0].market.datetime;
            let end = bars[bars.len() - 1].market.datetime;

            list.push(Arc::new(Contract {
                start,
                end,
                metadata: (**meta).clone(),
                table,
                bar: Arc::new(bars),
                profit,
            }));
        }

        let start = *index_table.first().ok_or_else(|| io::Error::other("数据源中没有行情数据"))?;
        let end = *index_table.last().ok_or_else(|| io::Error::other("数据源中没有行情数据"))?;

        Ok(DataFrame {
            start,
            end,
            list,
            index: index_table.into_iter().collect(),
            sector: self.sector.clone(),
            indice: self.indice.clone(),
        })
    }
}

/// 读取单个股票的行情 JSON 文件（数组，每行一个交易日）。
fn load_market_rows(path: &Path) -> io::Result<Vec<MarketRow>> {
    let content = fs::read(path)?;
    serde_json::from_slice(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("解析行情文件 {} 失败: {e}", path.display())))
}

/// 读取成分股 JSON（分类名 -> 带后缀代码数组）。
fn load_groups(path: &Path) -> io::Result<HashMap<String, Vec<String>>> {
    let content = fs::read(path)?;
    serde_json::from_slice(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("解析成分股文件 {} 失败: {e}", path.display())))
}

/// 按日期范围过滤升序数据，返回 `None` 表示无交集。
fn filter_range<T: RowDate + Clone>(rows: &[T], range: Option<(Date, Date)>) -> Option<Vec<T>> {
    let Some((start, end)) = range else {
        return Some(rows.to_vec());
    };
    let data = rows
        .iter()
        .filter(|row| row.date() >= start && row.date() <= end)
        .cloned()
        .collect::<Vec<_>>();
    if data.is_empty() { None } else { Some(data) }
}

trait RowDate {
    fn date(&self) -> Date;
}

impl RowDate for Bar {
    fn date(&self) -> Date {
        self.market.datetime
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use tempfile::tempdir;
    use time::{Date, Month};

    use super::*;

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2025, Month::January, day).unwrap()
    }

    /// 写入新数据源格式的测试文件。
    fn write_market_json(path: &Path, rows: &[(&str, f64)]) {
        let mut content = String::from("[");
        for (i, (datetime, close)) in rows.iter().enumerate() {
            if i > 0 {
                content.push(',');
            }
            content.push_str(&format!(
                r#"{{"datetime":"{datetime}","change_pct":0.01,"open":{close:.1},"close":{close:.1},"high":{close:.1},"low":{close:.1},"volume":100.0,"amount":1000.0,"turnover":0.02,"is_st":false,"total_market":{total_market:.1}}}"#,
                total_market = close * 100.0
            ));
        }
        content.push(']');
        let mut file = fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    fn build_config(directory: &Path) -> Config {
        let market = directory.join("market");
        let metadata_path = directory.join("metadata.json");
        let sector_json = directory.join("sector.json");
        let indice_json = directory.join("indice.json");
        fs::create_dir_all(&market).unwrap();
        fs::write(
            &metadata_path,
            r#"{"000001.SZ":{"name":"测试股票","code":"000001.SZ","exchange":"深交所","listed_date":"2020-01-01"}}"#,
        )
        .unwrap();
        fs::write(&sector_json, r#"{"行业一":["000001.SZ"]}"#).unwrap();
        fs::write(&indice_json, r#"{"测试指数":["000001.SZ"]}"#).unwrap();
        write_market_json(
            &market.join("000001.SZ.json"),
            &[("2025-01-01", 10.0), ("2025-01-02", 11.0), ("2025-01-03", 12.0)],
        );

        Config {
            server: Default::default(),
            period: Vec::new(),
            data: crate::config::DataConfig {
                market,
                metadata: metadata_path,
                sector_json,
                indice_json,
                ..Default::default()
            },
        }
    }

    // 测试新 JSON 数据源加载为 DataFrame：行情/财务同源、收益与换手率正确。
    #[test]
    fn loads_new_json_data_source() {
        let directory = tempdir().unwrap();
        let config = build_config(directory.path());
        let db = DataFrameDb::from_config(&config).unwrap();
        let all_frame = db.query_all().unwrap();
        let frame = db.query(date(1), date(3)).unwrap();

        assert_eq!(all_frame.start, date(1));
        assert_eq!(all_frame.end, date(3));
        assert_eq!(all_frame.list.len(), 1);
        assert_eq!(all_frame.list[0].metadata.code.as_ref(), "000001");
        assert_eq!(all_frame.list[0].bar.len(), 3);
        assert_eq!(all_frame.list[0].bar[0].finance.total_market, 1000.0);
        assert_eq!(all_frame.list[0].bar[0].market.close, 10.0);

        // 收益：close 10/11/12，open 相同，前向收益为 0.1/0.0/1/11/1/11，换手率 0.02。
        assert_eq!(all_frame.list[0].profit.len(), 1);
        let [p1, p2, p3, p4, tr] = all_frame.list[0].profit[0];
        assert!((p1 - 0.1).abs() < 1e-12);
        assert!((p2 - 0.0).abs() < 1e-12);
        assert!((p3 - 1.0 / 11.0).abs() < 1e-12);
        assert!((p4 - 1.0 / 11.0).abs() < 1e-12);
        assert!((tr - 0.02).abs() < 1e-12);

        // 范围查询。
        assert_eq!(frame.list[0].bar.len(), 3);
        assert_eq!(frame.list.len(), 1);

        // 全列表与 members 合并回填。
        assert_eq!(all_frame.sector.len(), 1);
        assert!(all_frame.sector.contains("行业一"));
        assert_eq!(all_frame.indice.len(), 1);
        assert!(all_frame.indice.contains("测试指数"));
        assert!(all_frame.list[0].metadata.members.contains("行业一"));
        assert!(all_frame.list[0].metadata.members.contains("测试指数"));
    }

    // 测试范围过滤：只保留范围内数据，收益重新计算。
    #[test]
    fn filters_range_and_recomputes_profit() {
        let directory = tempdir().unwrap();
        let config = build_config(directory.path());
        let db = DataFrameDb::from_config(&config).unwrap();
        let frame = db.query(date(2), date(3)).unwrap();

        assert_eq!(frame.start, date(2));
        assert_eq!(frame.end, date(3));
        assert_eq!(frame.list[0].bar.len(), 2);
        assert_eq!(frame.list[0].bar[0].market.datetime.to_string(), "2025-01-02");
        assert!(frame.list[0].profit.is_empty());
    }

    // 测试行情文件缺失时跳过该合约，无任何数据时查询返回错误。
    #[test]
    fn skips_contract_with_missing_market_file() {
        let directory = tempdir().unwrap();
        let config = build_config(directory.path());
        fs::remove_file(config.data.market.join("000001.SZ.json")).unwrap();

        let db = DataFrameDb::from_config(&config).unwrap();
        assert!(db.query_all().is_err());
    }
}
