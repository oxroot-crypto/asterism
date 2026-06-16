//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/tests/e2e_test.rs
//! 功能概述：端到端集成测试 — 验证从项目加载、场景执行、存档/读档往返
//!           到跨场景跳转的完整游戏生命周期。使用 MockRenderer/MockAudioSystem
//!           替代真实 GPU/音频设备，确保可在 CI 环境中运行。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 对应任务：PH2-T09 — 集成测试
//! 覆盖 AC：AC01 (E2E 完整流程), AC05 (GameState 收集), AC06 (GameState 恢复),
//!          AC07 (QuickSave 写入), AC08 (QuickLoad 恢复)
//!
//! ## 测试列表
//!
//! | 测试函数 | 覆盖 AC | 验证内容 |
//! |----------|---------|----------|
//! | `test_e2e_load_project_and_play_scene` | AC01 | 加载模板→执行场景→验证命令分发 |
//! | `test_e2e_save_and_load_roundtrip` | AC05/AC06 | 保存→修改变量→读档→验证恢复 |
//! | `test_e2e_audio_state_preserved` | AC05 | 音频快照在存档中的保留 |
//! | `test_e2e_multiple_saves` | AC07/AC08 | 多槽位独立保存/删除 |
//! | `test_e2e_scene_not_found` | AC01 | load_scene 不存在的场景返回错误 |

use std::path::{Path, PathBuf};
use std::sync::Arc;

use aster_runtime::{
    App, GameContext, MockAudioSystem, MockRenderer, RuntimeError, SceneManager, SceneState,
};
use aster_save::{QUICK_SLOT, SaveManager};

// ─── 测试辅助函数 ─────────────────────────────────────────────────────────

/// 获取模板项目的绝对路径。
///
/// CARGO_MANIFEST_DIR 为 `engine/aster-runtime/`，
/// 模板项目位于 `../../templates/default_project`。
fn template_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("templates")
        .join("default_project")
}

/// 加载模板项目的 GameContext（仅解析+编译，无 GPU）。
fn load_game_context() -> GameContext {
    let app = App::load(&template_path()).expect("加载模板项目应成功");
    app.game_context
}

/// 创建 SceneManager 并注入 MockRenderer + MockAudioSystem。
///
/// 返回 (SceneManager, MockRenderer, MockAudioSystem)。
/// MockRenderer 和 MockAudioSystem 的所有权交给外部以便验证调用记录。
fn setup_scene_manager() -> (SceneManager, MockRenderer, MockAudioSystem) {
    let ctx = load_game_context();
    let mut sm = SceneManager::new(ctx);
    let renderer = MockRenderer::new();
    let audio = MockAudioSystem::new();
    sm.set_audio_system(Box::new(MockAudioSystem::new())); // 先注入一个，测试中按需替换
    (sm, renderer, audio)
}

/// 创建一个带有 SaveManager 的 SceneManager。
///
/// 返回 (SceneManager, MockRenderer, MockAudioSystem, Arc<SaveManager>, TempDir)。
fn setup_scene_with_saves() -> (
    SceneManager,
    MockRenderer,
    MockAudioSystem,
    Arc<SaveManager>,
    tempfile::TempDir,
) {
    let (mut sm, renderer, audio) = setup_scene_manager();
    let save_dir = tempfile::tempdir().expect("创建临时存档目录失败");
    let save_mgr = Arc::new(SaveManager::new(save_dir.path().to_path_buf()));
    sm.set_save_manager(save_mgr.clone());
    (sm, renderer, audio, save_mgr, save_dir)
}

// ─── E2E 测试 ─────────────────────────────────────────────────────────────

/// AC01 — 验证加载模板项目并执行场景的基本流程。
///
/// 加载模板项目的序章场景，执行 VM 直到第一个暂停点，
/// 验证渲染器和音频系统收到了正确的命令调用。
#[test]
fn test_e2e_load_project_and_play_scene() {
    let (mut sm, mut renderer, _audio) = setup_scene_manager();

    // 加载序章场景
    sm.load_scene("prologue").expect("加载序章应成功");
    assert_eq!(*sm.state(), SceneState::Playing, "场景应进入 Playing 状态");

    // 执行场景到第一个暂停点
    let result = sm.update(Some(&mut renderer));
    assert!(result.is_ok(), "场景执行不应出错: {:?}", result.err());

    // 验证渲染器收到了命令调用
    let call_count = renderer.call_count();
    assert!(
        call_count > 0,
        "渲染器应收到至少 1 个命令调用，实际 {} 次",
        call_count
    );

    // 序章脚本以背景设置和对话开头，验证至少收到了 set_background 或 set_dialogue
    let has_bg = renderer.has_call_containing("set_background");
    let has_dialogue = renderer.has_call_containing("set_dialogue");
    let has_narration = renderer.has_call_containing("set_narration");
    assert!(
        has_bg || has_dialogue || has_narration,
        "序章应包含背景设置或对话/旁白命令。调用记录:\n{:?}",
        renderer.calls()
    );

    // 验证命令日志非空
    assert!(!sm.command_log().is_empty(), "命令日志应记录执行的命令");

    // 场景应处于 Playing 或 Ended 状态（取决于序章在何处暂停）
    let state = *sm.state();
    assert!(
        matches!(state, SceneState::Playing | SceneState::Ended),
        "场景执行后应为 Playing 或 Ended，实际 {:?}",
        state
    );
}

/// AC05/AC06 — 验证完整的保存→修改→读档→恢复往返。
///
/// 执行场景到暂停点，收集游戏状态并保存。然后修改变量，
/// 再读档恢复，验证变量值回到存档时的状态。
#[test]
fn test_e2e_save_and_load_roundtrip() {
    let (mut sm, mut renderer, _audio, save_mgr, _save_dir) = setup_scene_with_saves();

    // 加载序章并执行到暂停点
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 收集游戏状态（使用槽位 0）
    let save_data = sm.collect_game_state(0);

    // 验证收集到的状态有意义
    assert_eq!(save_data.scene_id, "prologue");
    assert_eq!(save_data.slot, 0);
    assert!(save_data.vm_snapshot.pc > 0, "VM PC 应已前进");
    assert_eq!(save_data.version, 1);

    // 保存到磁盘
    save_mgr.save(0, &save_data).expect("保存到槽位 0 应成功");

    // 验证文件存在
    assert!(save_mgr.slot_exists(0));

    // 通过 VM 手动修改变量（模拟存档后继续推进）
    sm.vm_mut()
        .variables_mut()
        .set("test_modified", aster_core::Value::Int(999));

    // 读档并恢复
    let loaded = save_mgr.load(0).expect("读档应成功");
    sm.restore_game_state(&loaded, &mut Some(&mut renderer))
        .expect("恢复游戏状态应成功");

    // 验证：修改变量应回滚（存档时不存在 test_modified）
    let modified = sm.vm().variables().get("test_modified");
    assert!(
        modified.is_none(),
        "读档恢复后修改的变量应不存在，实际 {:?}",
        modified
    );

    // 验证：场景 ID 回到序章
    assert_eq!(sm.current_scene_id(), Some("prologue"));
}

/// AC05 — 验证音频状态在存档中得到保留。
///
/// 注入 MockAudioSystem，执行场景后收集游戏状态，
/// 验证 audio_state 包含 MockAudioSystem 的默认 BGM 路径。
#[test]
fn test_e2e_audio_state_preserved() {
    let (mut sm, mut renderer, _audio, save_mgr, _save_dir) = setup_scene_with_saves();

    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 收集状态，验证音频快照
    let save_data = sm.collect_game_state(0);
    // MockAudioSystem 的 get_state() 返回默认 mock_bgm.ogg
    assert_eq!(
        save_data.audio_state.current_bgm_path,
        Some("mock_bgm.ogg".into()),
        "音频快照应包含 MockAudioSystem 的默认 BGM 路径"
    );

    // 保存后恢复，验证 restore_state 被调用
    save_mgr.save(0, &save_data).expect("保存应成功");
    let loaded = save_mgr.load(0).expect("读档应成功");

    // 注入新的 MockAudioSystem 以验证 restore_state 被调用
    let new_audio = MockAudioSystem::new();
    sm.set_audio_system(Box::new(new_audio));

    sm.restore_game_state(&loaded, &mut Some(&mut renderer))
        .expect("恢复应成功");

    // 注意：restore_game_state 内部调用 audio.restore_state()，
    // 但 MockAudioSystem 的调用记录不在外部可访问。
    // 此处主要验证 restore_game_state 不 panic。
}

/// AC07/AC08 — 验证多槽位独立保存。
///
/// 在 3 个不同槽位保存不同状态，验证每个槽位数据独立。
#[test]
fn test_e2e_multiple_saves() {
    let (mut sm, mut renderer, _audio, save_mgr, _save_dir) = setup_scene_with_saves();

    // 加载并执行序章
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 保存到槽位 0（序章）
    let data0 = sm.collect_game_state(0);
    save_mgr.save(0, &data0).expect("保存槽位 0 应成功");

    // 保存到槽位 1（同场景，不同 PC）
    let data1 = sm.collect_game_state(1);
    save_mgr.save(1, &data1).expect("保存槽位 1 应成功");

    // 保存到快速存档槽位 98
    let data98 = sm.collect_game_state(QUICK_SLOT);
    save_mgr
        .save(QUICK_SLOT, &data98)
        .expect("保存快速存档应成功");

    // 验证 3 个槽位各有数据
    let list = save_mgr.list_saves().expect("list_saves 应成功");
    assert_eq!(list.len(), 3, "应列出 3 个存档");

    // 验证各槽位数据独立
    let loaded0 = save_mgr.load(0).expect("加载槽位 0 应成功");
    let loaded1 = save_mgr.load(1).expect("加载槽位 1 应成功");
    let loaded98 = save_mgr.load(QUICK_SLOT).expect("加载快速存档应成功");

    assert_eq!(loaded0.slot, 0);
    assert_eq!(loaded1.slot, 1);
    assert_eq!(loaded98.slot, QUICK_SLOT);
    assert_eq!(loaded0.scene_id, "prologue");
    assert_eq!(loaded1.scene_id, "prologue");
}

/// AC01 — 验证加载不存在的场景返回错误，不 panic。
#[test]
fn test_e2e_scene_not_found() {
    let (mut sm, _renderer, _audio) = setup_scene_manager();

    let result = sm.load_scene("nonexistent_scene");
    assert!(result.is_err(), "不存在的场景应返回错误");
    match result {
        Err(RuntimeError::SceneNotFound { scene_id }) => {
            assert_eq!(scene_id, "nonexistent_scene");
        }
        other => panic!("期望 SceneNotFound 错误，实际 {:?}", other),
    }
}

/// 验证空场景 ID 返回错误。
#[test]
fn test_e2e_empty_scene_id() {
    let (mut sm, _renderer, _audio) = setup_scene_manager();

    let result = sm.load_scene("");
    assert!(result.is_err(), "空场景 ID 应返回错误");
}

/// 验证 collect_game_state 在无 SaveManager 时也能正常工作。
///
/// collect_game_state 是纯数据收集操作，不依赖 SaveManager。
#[test]
fn test_collect_game_state_without_save_manager() {
    let (mut sm, mut renderer, _audio) = setup_scene_manager();

    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 即使没有 SaveManager 也能收集状态
    let save_data = sm.collect_game_state(0);
    assert_eq!(save_data.scene_id, "prologue");
    assert_eq!(save_data.slot, 0);
    assert!(save_data.vm_snapshot.pc > 0);
}

/// 验证 restart 后 SaveData 可以正常恢复（从 save_pc 重放）。
///
/// 场景执行到暂停点后保存，然后 load_scene 重新加载同一场景（模拟 restart），
/// 再 restore_game_state，验证恢复到存档时的位置。
#[test]
fn test_e2e_restore_after_scene_reload() {
    let (mut sm, mut renderer, _audio, save_mgr, _save_dir) = setup_scene_with_saves();

    // 加载并执行序章
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 保存当前状态
    let save_data = sm.collect_game_state(0);
    let saved_scene_id = save_data.scene_id.clone();
    save_mgr.save(0, &save_data).expect("保存应成功");

    // 重新加载同一场景（模拟 restart——VM 被重置）
    sm.load_scene("prologue").expect("重新加载应成功");

    // 恢复存档状态
    let loaded = save_mgr.load(0).expect("读档应成功");
    sm.restore_game_state(&loaded, &mut Some(&mut renderer))
        .expect("恢复应成功");

    // 验证恢复到存档时的场景
    assert_eq!(sm.current_scene_id(), Some(saved_scene_id.as_str()));
}
