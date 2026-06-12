//! Asterism IDE — Tauri v2 构建脚本
//!
//! 文件路径：ide/src-tauri/build.rs
//! 功能概述：Tauri 构建脚本 — 在编译期由 Cargo 执行，
//!           负责生成前端资源嵌入代码和平台配置
//! 作者：Claude (AI)
//! 创建日期：2026-06-13

/// Cargo 构建脚本入口 — 调用 tauri_build 生成资源绑定
fn main() {
    tauri_build::build()
}
