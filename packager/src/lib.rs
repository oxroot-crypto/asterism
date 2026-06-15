//! Asterism — 打包工具 CLI
//!
//! 文件路径：packager/src/lib.rs
//! 功能概述：库入口 — 导出 CLI 定义和执行函数，供 IDE 后端（Tauri Command）调用。
//!           Phase 0 定义三个子命令骨架（build / archive / init）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

use clap::{Parser, Subcommand};

/// Asterism 游戏项目打包与分发工具。
///
/// 负责将 `.aster` 脚本、角色定义、图片/音频/字体资源编译归档，
/// 生成可独立运行的平台安装包。
///
/// # 使用示例
///
/// ```bash
/// # 构建项目
/// aster-pack build ./my_galgame --release
///
/// # 归档资源（不解密，仅打包）
/// aster-pack archive ./my_galgame --output dist/
///
/// # 创建新项目骨架
/// aster-pack init ./my_new_project
/// ```
#[derive(Parser, Debug)]
#[command(
    name = "aster-pack",
    version = "0.1.0",
    about = "Asterism — Galgame 游戏项目打包与分发工具",
    long_about = "将 Asterism 游戏项目编译、归档、生成平台安装包。\n\n\
                  支持子命令：\n  build   — 编译脚本并生成安装包\n  \
                  archive — 归档资源文件\n  \
                  init    — 创建新项目骨架"
)]
pub struct Cli {
    /// 要执行的子命令
    #[command(subcommand)]
    pub command: Commands,
}

/// 可用的子命令集合。
///
/// 每个子命令对应打包流程的一个独立阶段：
/// - `Build`: 编译 .aster 脚本 → .asterbyte + 生成平台安装包
/// - `Archive`: 将资源文件压缩归档为 .asterarchive
/// - `Init`: 从模板创建新游戏项目目录结构
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 编译脚本并生成平台安装包
    ///
    /// 执行流程：
    /// 1. 编译所有 .aster 脚本 → .asterbyte
    /// 2. 复制并优化资源文件
    /// 3. 嵌入引擎运行时二进制
    /// 4. 调用平台打包工具生成安装包（NSIS/DMG/AppImage）
    Build {
        /// 项目根目录路径
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: String,

        /// 是否为正式发布构建（开启优化和压缩）
        #[arg(short, long, default_value_t = false)]
        release: bool,
    },

    /// 归档资源文件为 .asterarchive
    ///
    /// 将图片、音频、字体等游戏资源打包为单一归档文件，
    /// 可选 AES-256-GCM 加密。
    Archive {
        /// 项目根目录路径
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: String,

        /// 输出目录（默认 dist/）
        #[arg(short, long, default_value = "dist")]
        output: String,
    },

    /// 创建新项目骨架
    ///
    /// 从内置模板生成标准项目目录结构，包含：
    /// - aster.toml（游戏元数据）
    /// - scripts/prologue.aster（示例场景）
    /// - characters/（角色定义目录）
    /// - assets/、gui/、fonts/ 等资源目录
    Init {
        /// 新项目的目标路径
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: String,
    },
}

/// 执行 CLI 命令的核心分发函数。
///
/// 根据解析出的子命令执行对应的处理逻辑。
/// Phase 0：所有子命令打印 TODO 信息并返回 Ok。
///
/// # 参数
/// - `cli`: 解析完成的命令行参数结构体
///
/// # 返回值
/// - `Ok(())`: 命令执行成功
/// - `Err(anyhow::Error)`: 执行失败（Phase 1+ 实现）
pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Build {
            project_dir,
            release,
        } => {
            println!("📦 build 命令 — 待实现 (Phase 6)");
            println!("   项目目录: {project_dir}");
            println!("   发布构建: {release}");
            println!("   TODO: 编译 .aster → .asterbyte，生成平台安装包");
        }
        Commands::Archive {
            project_dir,
            output,
        } => {
            println!("📦 archive 命令 — 待实现 (Phase 6)");
            println!("   项目目录: {project_dir}");
            println!("   输出目录: {output}");
            println!("   TODO: 归档资源文件为 .asterarchive");
        }
        Commands::Init { project_dir } => {
            println!("📦 init 命令 — 待实现 (Phase 3)");
            println!("   目标路径: {project_dir}");
            println!("   TODO: 从模板创建新项目骨架");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 占位测试 — 确认测试框架可用
    #[test]
    fn test_cli_struct_exists() {
        // 验证 Cli 结构体可以正常构造
        let cli = Cli {
            command: Commands::Init {
                project_dir: "./test_project".into(),
            },
        };
        // 验证 run 函数正常返回
        assert!(run(cli).is_ok());
    }

    /// 验证 build 子命令可以正常执行
    #[test]
    fn test_build_command_runs() {
        let cli = Cli {
            command: Commands::Build {
                project_dir: "./test_project".into(),
                release: true,
            },
        };
        assert!(run(cli).is_ok());
    }

    /// 验证 archive 子命令可以正常执行
    #[test]
    fn test_archive_command_runs() {
        let cli = Cli {
            command: Commands::Archive {
                project_dir: "./test_project".into(),
                output: "dist".into(),
            },
        };
        assert!(run(cli).is_ok());
    }
}
