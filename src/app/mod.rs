mod parse;
mod run;
mod stlist;
mod test;

use clap::Parser;

pub use parse::ParseCommand;
pub use run::RunCommand;
pub use stlist::StListCommand;

#[cfg(test)]
pub(crate) use run::build_openapi;
pub use test::TestCommand;

/// 因子分析命令行入口。
#[derive(Debug, Parser)]
#[command(name = "factor-analysis", version, about = "因子分析服务")]
pub enum App {
    /// 解析 Tbf 数据。
    Parse(ParseCommand),
    /// 运行 Web 服务。
    Run(RunCommand),
    /// 按日期导出全部 ST 行情。
    Stlist(StListCommand),
    /// 检查数据库并导出文本数据。
    Test(TestCommand),
}

impl App {
    /// 执行解析后的子命令。
    pub async fn execute(self) {
        match self {
            Self::Parse(command) => command.execute(),
            Self::Run(command) => command.execute().await,
            Self::Stlist(command) => command.execute(),
            Self::Test(command) => command.execute(),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    // 测试 parse 子命令可以通过 Clap 派生参数完成解析。
    #[test]
    fn parses_parse_command() {
        let app = App::try_parse_from(["factor-analysis", "parse"]).unwrap();

        assert!(matches!(app, App::Parse(_)));
    }

    // 测试 run 子命令可以通过 Clap 派生参数完成解析。
    #[test]
    fn parses_run_command() {
        let app = App::try_parse_from(["factor-analysis", "run"]).unwrap();

        assert!(matches!(app, App::Run(_)));
    }

    // 测试 stlist 子命令要求显式指定输出目录。
    #[test]
    fn parses_stlist_command_with_required_output() {
        let app = App::try_parse_from(["factor-analysis", "stlist", "output"]).unwrap();

        assert!(matches!(app, App::Stlist(_)));
        assert!(App::try_parse_from(["factor-analysis", "stlist"]).is_err());
    }

    // 测试 test 子命令可以通过 Clap 派生参数完成解析。
    #[test]
    fn parses_test_command() {
        let app = App::try_parse_from(["factor-analysis", "test"]).unwrap();

        assert!(matches!(app, App::Test(_)));
    }
}
