use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

use time::Date;

use crate::{config::Config, db::MarketDataDb};

/// 按日期导出数据库中的全部 ST 行情。
#[derive(Debug, clap::Args)]
pub struct StListCommand {
    /// 输出目录；每个交易日写入 st-{日期}.txt。
    #[arg(value_name = "OUTPUT")]
    pub output: PathBuf,
}

impl StListCommand {
    pub(super) fn execute(self) {
        let config = Config::load_or_gen_default();
        if let Err(error) = self.execute_with(&config) {
            eprintln!("导出 ST 数据失败: {error}");
            std::process::exit(1);
        }
    }

    fn execute_with(&self, config: &Config) -> io::Result<()> {
        let records = collect_st_records(&config.data.market)?;
        fs::create_dir_all(&self.output)?;

        for (date, records) in &records {
            let path = self.output.join(format!("st-{date}.txt"));
            let mut output = BufWriter::new(File::create(&path)?);
            for code in records {
                writeln!(output, "{code}")?;
            }
            output.flush()?;
        }

        println!("ST 数据已输出到: {}，共 {} 天", self.output.display(), records.len());
        Ok(())
    }
}

fn collect_st_records(database_dir: &Path) -> io::Result<BTreeMap<Date, Vec<String>>> {
    let mut records = BTreeMap::<Date, Vec<String>>::new();
    for database_path in stock_databases(database_dir)? {
        let code = stock_code(&database_path)?;
        let database = MarketDataDb::open_read_only(&database_path).map_err(io::Error::other)?;
        for data in database.query_all().map_err(io::Error::other)? {
            if data.is_st {
                records.entry(data.datetime).or_default().push(code.clone());
            }
        }
    }
    Ok(records)
}

fn stock_databases(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut databases = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    databases.retain(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "db"));
    databases.sort();
    Ok(databases)
}

fn stock_code(database_path: &Path) -> io::Result<String> {
    database_path
        .file_stem()
        .and_then(|code| code.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("数据库文件名不是有效的股票代码: {}", database_path.display()),
            )
        })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use time::{Date, format_description::well_known::Iso8601};

    use crate::{
        config::{DataConfig, ServerConfig},
        db::MarketData,
    };

    use super::*;

    fn market(date: &str, is_st: bool) -> MarketData {
        MarketData {
            datetime: Date::parse(date, &Iso8601::DATE).unwrap(),
            change_percent: 0.01,
            open: 10.0,
            close: 11.0,
            high: 12.0,
            low: 9.0,
            volume: 100.0,
            turnover: 1_000.0,
            turnover_rate: 0.02,
            is_st,
        }
    }

    #[test]
    fn exports_st_records_grouped_by_date() {
        let directory = tempdir().unwrap();
        let market_dir = directory.path().join("market");
        let output_dir = directory.path().join("output");
        fs::create_dir_all(&market_dir).unwrap();

        let mut first = MarketDataDb::new(market_dir.join("000001.db")).unwrap();
        first
            .replace_all(&[market("2025-01-01", true), market("2025-01-02", false), market("2025-01-03", true)])
            .unwrap();
        let mut second = MarketDataDb::new(market_dir.join("000002.db")).unwrap();
        second.replace_all(&[market("2025-01-01", true), market("2025-01-02", true)]).unwrap();

        let config = Config {
            server: ServerConfig::default(),
            period: Vec::new(),
            data: DataConfig {
                market: market_dir,
                ..DataConfig::default()
            },
        };
        StListCommand { output: output_dir.clone() }.execute_with(&config).unwrap();

        let first_day = fs::read_to_string(output_dir.join("st-2025-01-01.txt")).unwrap();
        let second_day = fs::read_to_string(output_dir.join("st-2025-01-02.txt")).unwrap();
        let third_day = fs::read_to_string(output_dir.join("st-2025-01-03.txt")).unwrap();
        assert_eq!(first_day, "000001\n000002\n");
        assert_eq!(second_day, "000002\n");
        assert_eq!(third_day, "000001\n");
    }
}
