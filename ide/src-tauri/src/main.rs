//! Asterism IDE — Tauri v2 Rust 后端
//!
//! 文件路径：ide/src-tauri/src/main.rs
//! 功能概述：Tauri 应用入口点 — Windows 平台隐藏控制台窗口，委派给 lib::run()
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

// 在 Windows release 构建中不弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 程序入口 — 委派给库 crate 的 `run()` 函数
fn main() {
    aster_ide_lib::run()
}
