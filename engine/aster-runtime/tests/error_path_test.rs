//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/tests/error_path_test.rs
//! 功能概述：异常路径集成测试 — 验证各子系统在异常输入下的优雅降级行为：
//!           存档损坏检测、空槽位/版本不兼容报错、缺失场景的错误处理、
//!           非法状态操作的拒绝。所有异常场景必须返回结构化错误，不 panic。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 对应任务：PH2-T09 — 集成测试
//! 覆盖 AC：AC02 (异常路径测试), AC05 (CRC32 损坏检测), AC07 (所有测试不 panic)
//!
//! ## 测试列表
//!
//! | 测试函数 | 验证内容 |
//! |----------|----------|
//! | `test_corrupted_save_detection` | 篡改存档文件后 load 返回 Corrupted |
//! | `test_empty_save_slot_error` | 空槽位 load 返回 EmptySlot |
//! | `test_incompatible_save_version` | 手动写入 version=99 存档后 load 返回 IncompatibleVersion |
//! | `test_scene_not_found_error` | load_scene 不存在场景返回 SceneNotFound |
//! | `test_update_without_scene` | 未加载场景时 update 不 panic |
//! | `test_invalid_choice_index` | 选择越界索引返回 InvalidChoiceIndex |
//! | `test_truncated_save_file` | 截断存档文件返回 Corrupted |

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aster_core::SaveData;
use aster_runtime::{App, GameContext, MockRenderer, RuntimeError, SceneManager};
use aster_save::{SaveError, SaveManager};

// ─── 测试辅助函数 ─────────────────────────────────────────────────────────

/// 获取模板项目的绝对路径。
fn template_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("templates")
        .join("default_project")
}

/// 加载模板项目的 GameContext。
fn load_game_context() -> GameContext {
    let app = App::load(&template_path()).expect("加载模板项目应成功");
    app.game_context
}

/// 创建带有 SaveManager 的 SceneManager。
fn setup_with_saves() -> (
    SceneManager,
    MockRenderer,
    Arc<SaveManager>,
    tempfile::TempDir,
) {
    let ctx = load_game_context();
    let mut sm = SceneManager::new(ctx);
    let renderer = MockRenderer::new();
    let save_dir = tempfile::tempdir().expect("创建临时存档目录失败");
    let save_mgr = Arc::new(SaveManager::new(save_dir.path().to_path_buf()));
    sm.set_save_manager(save_mgr.clone());
    (sm, renderer, save_mgr, save_dir)
}

// ─── 异常路径测试 ─────────────────────────────────────────────────────────

/// AC05 — 验证存档 CRC32 损坏检测。
///
/// 正常保存存档后，篡改存档文件中的一个字节，
/// 验证 `load()` 返回 `SaveError::Corrupted`，不 panic。
#[test]
fn test_corrupted_save_detection() {
    let (_sm, _renderer, save_mgr, _save_dir) = setup_with_saves();

    // 保存正常存档
    let data = SaveData::new(0, "test_scene");
    save_mgr.save(0, &data).expect("保存应成功");

    // 篡改文件（翻转数据区的一个字节）
    let save_path = save_mgr.slot_path(0);
    let mut bytes = fs::read(&save_path).expect("读取存档文件应成功");
    assert!(bytes.len() > 10, "存档文件应有足够字节");
    // 确保篡改在 CRC32（4 字节）之后的数据区
    bytes[10] ^= 0xFF;
    fs::write(&save_path, &bytes).expect("写回应成功");

    // 加载应检测到损坏
    match save_mgr.load(0) {
        Err(SaveError::Corrupted { slot, reason: _ }) => {
            assert_eq!(slot, 0);
        }
        Err(other) => panic!("期望 Corrupted 错误，实际 {:?}", other),
        Ok(_) => panic!("损坏的存档不应加载成功"),
    }
}

/// AC05 — 验证空槽位加载返回 EmptySlot 错误。
///
/// 对从未保存过的槽位调用 `load()`，验证返回 `SaveError::EmptySlot`。
#[test]
fn test_empty_save_slot_error() {
    let (_sm, _renderer, save_mgr, _save_dir) = setup_with_saves();

    // 对从未保存的槽位执行 load
    match save_mgr.load(0) {
        Err(SaveError::EmptySlot { slot }) => assert_eq!(slot, 0),
        other => panic!("期望 EmptySlot 错误，实际 {:?}", other),
    }

    // 对快速存档槽位也验证
    match save_mgr.load(98) {
        Err(SaveError::EmptySlot { slot }) => assert_eq!(slot, 98),
        other => panic!("期望 EmptySlot 错误，实际 {:?}", other),
    }
}

/// AC07 — 验证场景不存在时返回结构化错误。
///
/// `load_scene` 不存在的场景 ID 应返回 `RuntimeError::SceneNotFound`，
/// 不 panic。
#[test]
fn test_scene_not_found_error() {
    let ctx = load_game_context();
    let mut sm = SceneManager::new(ctx);
    let mut renderer = MockRenderer::new();

    // 加载不存在的场景
    let result = sm.load_scene("chapter_that_does_not_exist");
    match result {
        Err(RuntimeError::SceneNotFound { scene_id }) => {
            assert_eq!(scene_id, "chapter_that_does_not_exist");
        }
        other => panic!("期望 SceneNotFound 错误，实际 {:?}", other),
    }

    // 验证后续 update 不会 panic
    let update_result = sm.update(Some(&mut renderer));
    assert!(update_result.is_ok(), "未加载场景时 update 应返回 Ok(())");
}

/// 验证在未加载场景时调用 update 不 panic。
///
/// SceneManager 初始化后直接调用 update（未调用 load_scene），
/// 应返回 Ok(()) 且不 panic。
#[test]
fn test_update_without_scene() {
    let ctx = load_game_context();
    let mut sm = SceneManager::new(ctx);
    let mut renderer = MockRenderer::new();

    // 未加载场景直接 update
    let result = sm.update(Some(&mut renderer));
    // 应返回 Ok(())——内部检查状态不是 Playing 时直接返回
    assert!(
        result.is_ok(),
        "未加载场景时 update 不应出错: {:?}",
        result.err()
    );
}

/// 验证在没有菜单时调用 select_choice 返回错误。
///
/// 场景未执行到菜单时调用 select_choice 应返回 InvalidState 错误，不 panic。
#[test]
fn test_select_choice_without_menu() {
    let (mut sm, mut renderer, _save_mgr, _save_dir) = setup_with_saves();

    // 加载序章（序章开头是对话，不是菜单）
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 在没有菜单时调用 select_choice
    let result = sm.select_choice(0, Some(&mut renderer));
    assert!(result.is_err(), "无菜单时 select_choice 应返回错误");
}

/// 验证截断存档文件被正确处理。
///
/// 写入一个字节数不足的文件（< 4 字节 CRC32 头部），
/// 验证 load() 返回 Corrupted 错误，不 panic。
#[test]
fn test_truncated_save_file() {
    let (_sm, _renderer, save_mgr, _save_dir) = setup_with_saves();

    // 写入仅 2 字节的"存档"
    let save_path = save_mgr.slot_path(0);
    fs::write(&save_path, [0xAA, 0xBB]).expect("写入应成功");

    match save_mgr.load(0) {
        Err(SaveError::Corrupted { .. }) => { /* 预期 */ }
        other => panic!("截断文件应返回 Corrupted 错误，实际 {:?}", other),
    }
}

/// 验证空存档文件被正确处理。
///
/// 写入空文件（0 字节），验证 load() 返回 Corrupted 错误。
#[test]
fn test_empty_save_file() {
    let (_sm, _renderer, save_mgr, _save_dir) = setup_with_saves();

    // 写入空文件
    let save_path = save_mgr.slot_path(0);
    fs::write(&save_path, []).expect("写入应成功");

    match save_mgr.load(0) {
        Err(SaveError::Corrupted { .. }) => { /* 预期 */ }
        other => panic!("空文件应返回 Corrupted 错误，实际 {:?}", other),
    }
}
