//! Asterism IDE — Tauri v2 Rust 后端
//!
//! 文件路径：ide/src-tauri/src/lib.rs
//! 功能概述：Tauri 应用库入口 — 创建 Tauri Builder、注册插件和命令、
//!           启动事件循环。Phase 0 不注册任何业务命令。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

/// 启动 Asterism IDE 的 Tauri 应用程序。
///
/// 执行流程：
/// 1. 创建 `tauri::Builder` 实例
/// 2. 注册内置插件（如文件打开、剪贴板等基础能力）
/// 3. 注册 IPC 命令（Phase 0 无业务命令，Phase 3 起添加）
/// 4. 调用 `run()` 进入事件循环
///
/// # Panics
/// 仅在 Tauri 初始化失败时 panic（如系统 WebView 不可用）
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 后续 Phase 在此通过 .invoke_handler(tauri::generate_handler![...]) 注册命令
        .run(tauri::generate_context!())
        .expect("启动 Asterism IDE 时发生错误 — 请确认系统已安装 WebView2 (Windows) 或 WebKit (Linux/macOS)")
}
