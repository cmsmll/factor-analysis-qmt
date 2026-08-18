/// 数据源检查命令（暂未实现）。
#[derive(Debug, clap::Args)]
pub struct TestCommand {}

impl TestCommand {
    pub(super) fn execute(self) {
        println!("test 子命令暂未实现");
    }
}
