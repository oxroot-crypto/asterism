//! Asterism — 打包工具 CLI
//!
//! 文件路径：packager/src/main.rs
//! 功能概述：CLI 入口 — 解析命令行参数，分派到对应子命令处理函数。
//!           Phase 0 仅搭建 CLI 框架，所有子命令打印 TODO 后返回。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

use aster_pack::{Cli, run};
use clap::Parser;

/// 程序入口 — 解析参数并执行
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli)
}
