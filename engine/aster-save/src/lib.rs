//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-save/src/lib.rs
//! 功能概述：存档系统 — 管理游戏存档的创建、读取、列表和删除。
//!           存档文件格式为 `[4 字节 CRC32 LE] + [MessagePack 序列化的 SaveData]`，
//!           确保存档完整性可验证。支持 5 个手动槽位（0-4）、
//!           1 个快速存档槽位（98）和 1 个自动存档槽位（99）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::save::{SaveData, SaveSlotInfo}（存档数据结构）
//! - rmp_serde（MessagePack 序列化/反序列化）
//! - crc32fast（CRC32 校验和计算）
//!
//! 架构位置：aster-core ← aster-save
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `save_manager` | `save_manager.rs` | SaveManager 结构体 — 存档 CRUD + CRC32 校验 + MessagePack 序列化 |
//!
//! ## 库存档槽位分配
//!
//! | 槽位 | 类型 | 常量 |
//! |------|------|------|
//! | 0 ~ 4 | 手动存档 | `MANUAL_SLOT_COUNT = 5` |
//! | 98 | 快速存档 | `QUICK_SLOT = 98` |
//! | 99 | 自动存档 | `AUTO_SLOT = 99` |
//!
//! ## 对应 Phase 2 任务
//!
//! - **PH2-T06**（本任务）：SaveData 数据结构 + SaveManager 基础读写 + CRC32
//! - **PH2-T07**（后续）：槽位管理 UI + 缩略图捕获

// 模块声明
pub mod save_manager;

// 重导出所有公开类型
pub use save_manager::{
    AUTO_SLOT, CURRENT_VERSION, MANUAL_SLOT_COUNT, QUICK_SLOT, SaveError, SaveManager,
};
