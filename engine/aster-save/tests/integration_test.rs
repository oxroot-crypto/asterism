//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-save/tests/integration_test.rs
//! 功能概述：存档系统集成测试 — 验证 SaveManager 的完整读写往返、CRC32 完整性校验、
//!           多槽位隔离、列表准确性。所有测试使用临时目录，测试结束后自动清理。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 对应任务：PH2-T09 — 集成测试
//! 覆盖 AC：AC02 (CRC32 损坏检测), AC04 (存档集成测试), AC05 (CRC32 损坏检测正确)
//!
//! ## 测试列表
//!
//! | 测试函数 | 覆盖 AC | 验证内容 |
//! |----------|---------|----------|
//! | `test_save_load_full_state` | AC04 | 含所有嵌套结构的 SaveData 完整往返 |
//! | `test_crc32_integrity` | AC02 | 篡改文件后 load 返回 Corrupted |
//! | `test_multiple_slots_isolation` | AC04 | 5 个槽位数据不交叉污染 |
//! | `test_list_saves_accuracy` | AC04 | 增删操作后 list 准确反映磁盘状态 |

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use aster_core::{
    AudioSnapshot, CallFrameSnapshot, FlagSet, RenderState, SaveData, Value, VariableStore,
    VmSnapshot,
};
use aster_save::{MANUAL_SLOT_COUNT, QUICK_SLOT, SaveError, SaveManager};

// ─── 测试辅助函数 ─────────────────────────────────────────────────────────

/// 创建临时目录并初始化 SaveManager，返回 (SaveManager, TempDir)。
///
/// TempDir 在函数退出时自动删除，确保测试间无状态泄漏。
fn setup_manager() -> (SaveManager, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let manager = SaveManager::new(dir.path().to_path_buf());
    (manager, dir)
}

/// 构造一个包含所有嵌套结构的完整 SaveData，用于往返测试。
///
/// # 参数
/// - `slot`: 槽位编号
/// - `scene_id`: 场景标识符
///
/// # 返回值
/// 包含变量、旗标、VM 快照、音频状态、渲染状态的完整存档数据。
fn full_save_data(slot: u8, scene_id: &str) -> SaveData {
    // 构造 VariableStore — 覆盖 Int/String/Float 三种类型
    let mut variables = VariableStore::new();
    variables.set("score", Value::Int(100));
    variables.set("player_name", Value::String("测试角色".into()));
    variables.set("progress", Value::Float(0.75));

    // 构造 FlagSet — 设置两个旗标
    let mut flags = FlagSet::new();
    flags.set("completed_ch1");
    flags.set("met_akane");

    // 构造 VmSnapshot — PC=128, R0=42, R1=3.14, 含一个调用帧, 栈含 3 个值
    let mut registers: [Value; 16] = [
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
    ];
    registers[0] = Value::Int(42);
    registers[1] = Value::Float(std::f64::consts::PI);

    let vm_snapshot = VmSnapshot {
        pc: 128,
        registers,
        call_stack: vec![CallFrameSnapshot {
            return_pc: 200,
            saved_registers: [
                Value::Int(10),
                Value::Int(20),
                Value::Int(30),
                Value::Int(40),
            ],
        }],
        stack: vec![Value::Int(1), Value::Int(2), Value::Int(3)],
    };

    // 构造 AudioSnapshot
    let audio_state = AudioSnapshot {
        current_bgm_path: Some("assets/bgm/test.ogg".into()),
        bgm_position_secs: 42.5,
        bgm_looping: true,
        bgm_volume: 0.7,
        se_volume: 0.3,
    };

    // 构造 RenderState
    let render_state = RenderState {
        current_bg: Some("assets/bg/classroom".into()),
        displayed_sprites: Vec::new(),
    };

    SaveData {
        version: SaveData::CURRENT_VERSION,
        slot,
        timestamp: "2026-06-16T12:00:00+08:00".into(),
        scene_id: scene_id.into(),
        label: Some("scene_start".into()),
        vm_snapshot,
        variables,
        flags,
        audio_state,
        render_state,
    }
}

// ─── 集成测试 ─────────────────────────────────────────────────────────────

/// AC04 — 验证完整 SaveData 的保存→读取往返。
///
/// 构造包含所有嵌套结构的 SaveData，通过 SaveManager 保存后立即加载，
/// 逐字段比对确保序列化/反序列化和文件 I/O 的正确性。
#[test]
fn test_save_load_full_state() {
    let (manager, _dir) = setup_manager();

    // 保存完整存档到槽位 0
    let data = full_save_data(0, "chapter1/test");
    let info = manager.save(0, &data).expect("保存应成功");

    // 验证返回的槽位摘要
    assert_eq!(info.slot, 0);
    assert_eq!(info.scene_id, "chapter1/test");
    assert!(!info.has_thumbnail); // 未保存缩略图

    // 加载并逐字段验证
    let loaded = manager.load(0).expect("加载应成功");

    // 顶层字段
    assert_eq!(loaded.version, SaveData::CURRENT_VERSION);
    assert_eq!(loaded.slot, 0);
    assert_eq!(loaded.timestamp, "2026-06-16T12:00:00+08:00");
    assert_eq!(loaded.scene_id, "chapter1/test");
    assert_eq!(loaded.label, Some("scene_start".into()));

    // VM 快照
    assert_eq!(loaded.vm_snapshot.pc, 128);
    assert_eq!(loaded.vm_snapshot.registers[0], Value::Int(42));
    // Float 比较使用近似相等（f64 浮点误差）
    // 注意：Value 不实现 Copy，需要通过引用匹配
    match &loaded.vm_snapshot.registers[1] {
        Value::Float(f) => assert!((f - std::f64::consts::PI).abs() < 1e-10),
        other => panic!("期望 Float(PI)，实际 {:?}", other),
    }
    assert_eq!(loaded.vm_snapshot.call_stack.len(), 1);
    assert_eq!(loaded.vm_snapshot.call_stack[0].return_pc, 200);
    assert_eq!(loaded.vm_snapshot.stack.len(), 3);

    // 变量
    assert_eq!(loaded.variables.get("score"), Some(&Value::Int(100)));
    assert_eq!(
        loaded.variables.get("player_name"),
        Some(&Value::String("测试角色".into()))
    );
    assert_eq!(loaded.variables.get("progress"), Some(&Value::Float(0.75)));

    // 旗标
    assert!(loaded.flags.check("completed_ch1"));
    assert!(loaded.flags.check("met_akane"));
    assert!(!loaded.flags.check("never_set"));

    // 音频状态
    assert_eq!(
        loaded.audio_state.current_bgm_path,
        Some("assets/bgm/test.ogg".into())
    );
    assert!((loaded.audio_state.bgm_position_secs - 42.5).abs() < f64::EPSILON);
    assert!(loaded.audio_state.bgm_looping);
    assert!((loaded.audio_state.bgm_volume - 0.7).abs() < f32::EPSILON);
    assert!((loaded.audio_state.se_volume - 0.3).abs() < f32::EPSILON);

    // 渲染状态
    assert_eq!(
        loaded.render_state.current_bg,
        Some("assets/bg/classroom".into())
    );
    assert!(loaded.render_state.displayed_sprites.is_empty());

    // 验证文件确实在磁盘上
    let save_path = manager.slot_path(0);
    assert!(save_path.exists(), "存档文件应存在于磁盘");
    // CRC32（4 字节）+ MessagePack 数据应大于 4 字节
    let file_size = fs::metadata(&save_path).expect("应能获取文件大小").len();
    assert!(file_size > 4, "存档文件应包含 CRC32 + MessagePack 数据");
}

/// AC02 — 验证 CRC32 完整性校验：篡改存档文件后加载应返回 Corrupted 错误。
///
/// 正常保存后，翻转存档文件中间的一个字节，验证 load() 检测到损坏并返回
/// `SaveError::Corrupted`，而不是 panic 或返回错误的数据。
#[test]
fn test_crc32_integrity() {
    let (manager, _dir) = setup_manager();

    // 保存正常存档
    let data = full_save_data(0, "test_scene");
    manager.save(0, &data).expect("保存应成功");

    let save_path = manager.slot_path(0);

    // 读取并篡改文件：翻转数据区的第 10 个字节
    let mut file_bytes = fs::read(&save_path).expect("读取存档文件应成功");
    assert!(file_bytes.len() > 10, "存档文件应有足够字节供篡改");
    // 翻转数据区（CRC32 之后）的一个字节
    let tamper_idx = 10; // 确保在 CRC32（4 字节）之后
    file_bytes[tamper_idx] ^= 0xFF;
    fs::write(&save_path, &file_bytes).expect("写回篡改文件应成功");

    // 加载损坏的存档
    let result = manager.load(0);
    match result {
        Err(SaveError::Corrupted { slot, reason }) => {
            assert_eq!(slot, 0);
            assert!(!reason.is_empty(), "损坏错误应包含原因描述");
        }
        Err(other) => panic!("期望 Corrupted 错误，实际 {:?}", other),
        Ok(_) => panic!("损坏的存档不应加载成功"),
    }
}

/// AC02 补充 — 验证截断文件（< 4 字节）也检测为损坏。
///
/// 边界情况：文件太短以至于无法读取 CRC32 头部时，仍应返回 Corrupted 错误。
#[test]
fn test_crc32_truncated_file() {
    let (manager, _dir) = setup_manager();

    // 保存正常存档
    let data = full_save_data(0, "test_scene");
    manager.save(0, &data).expect("保存应成功");

    // 截断文件到 2 字节（< 4 字节 CRC32 头部大小）
    let save_path = manager.slot_path(0);
    fs::write(&save_path, [0x00, 0x01]).expect("写回截断文件应成功");

    // 加载截断文件应返回 Corrupted
    let result = manager.load(0);
    assert!(
        matches!(result, Err(SaveError::Corrupted { .. })),
        "截断文件应返回 Corrupted 错误，实际 {:?}",
        result
    );
}

/// AC04 — 验证多槽位隔离：不同槽位保存不同数据，加载时数据不交叉污染。
///
/// 在 5 个手动槽位中分别保存不同的 scene_id，验证每个槽位的数据独立。
/// 同时验证快速存档槽位（98）的行为与手动槽位一致。
#[test]
fn test_multiple_slots_isolation() {
    let (manager, _dir) = setup_manager();

    // 在 5 个手动槽位中保存不同数据
    for i in 0..MANUAL_SLOT_COUNT {
        let data = full_save_data(i, &format!("scene_{}", i));
        manager.save(i, &data).expect("保存应成功");
    }

    // 也在快速存档槽位保存
    let quick_data = full_save_data(QUICK_SLOT, "quick_scene");
    manager
        .save(QUICK_SLOT, &quick_data)
        .expect("快速存档应成功");

    // 验证每个槽位的数据独立
    for i in 0..MANUAL_SLOT_COUNT {
        let loaded = manager.load(i).expect("加载应成功");
        assert_eq!(
            loaded.scene_id,
            format!("scene_{}", i),
            "槽位 {} 的 scene_id 应独立",
            i
        );
        assert_eq!(loaded.slot, i);
    }

    // 验证快速存档槽位
    let quick_loaded = manager.load(QUICK_SLOT).expect("快速存档加载应成功");
    assert_eq!(quick_loaded.scene_id, "quick_scene");
    assert_eq!(quick_loaded.slot, QUICK_SLOT);

    // 验证文件路径各不相同
    let paths: Vec<PathBuf> = (0..MANUAL_SLOT_COUNT)
        .map(|i| manager.slot_path(i))
        .collect();
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            assert_ne!(paths[i], paths[j], "不同槽位应有不同的文件路径");
        }
    }
}

/// AC04 — 验证 `list_saves()` 在增删操作后准确反映磁盘状态。
///
/// 先验证空目录返回空列表，然后逐步添加、删除存档，
/// 每次操作后确认 list_saves 返回正确的槽位列表。
#[test]
fn test_list_saves_accuracy() {
    let (manager, _dir) = setup_manager();

    // 空目录：list_saves 应返回空列表
    let initial_list = manager.list_saves().expect("list_saves 应成功");
    assert!(
        initial_list.is_empty(),
        "空存档目录应返回空列表，实际 {} 条",
        initial_list.len()
    );

    // 保存 3 个槽位（跳过槽位 1 和 3，模拟非连续保存）
    for &slot in &[0, 2, 4] {
        let data = full_save_data(slot, &format!("scene_{}", slot));
        manager.save(slot, &data).expect("保存应成功");
    }

    // 验证 list_saves 返回 3 条记录
    let list = manager.list_saves().expect("list_saves 应成功");
    assert_eq!(list.len(), 3, "保存 3 个槽位后 list 应返回 3 条");

    // 验证槽位编号正确（按文件名排序，即数值升序）
    assert_eq!(list[0].slot, 0);
    assert_eq!(list[1].slot, 2);
    assert_eq!(list[2].slot, 4);

    // 验证每个条目的场景名正确
    for info in &list {
        assert_eq!(info.scene_id, format!("scene_{}", info.slot));
        assert!(!info.timestamp.is_empty(), "时间戳不应为空");
    }

    // 删除槽位 2
    manager.delete_save(2).expect("删除应成功");
    let list_after_delete = manager.list_saves().expect("list_saves 应成功");
    assert_eq!(list_after_delete.len(), 2, "删除 1 个后 list 应返回 2 条");
    assert_eq!(list_after_delete[0].slot, 0);
    assert_eq!(list_after_delete[1].slot, 4);

    // 验证槽位 2 的文件确实被删除
    assert!(!manager.slot_path(2).exists(), "槽位 2 的文件应被删除");
    // 验证槽位 0 和 4 文件仍存在
    assert!(manager.slot_path(0).exists());
    assert!(manager.slot_path(4).exists());

    // 验证 has_save() 方法
    assert!(manager.has_save(0));
    assert!(!manager.has_save(1)); // 从未保存
    assert!(!manager.has_save(2)); // 已删除
    assert!(!manager.has_save(3)); // 从未保存
    assert!(manager.has_save(4));
}

/// 验证对空槽位执行 delete_save 返回错误。
///
/// 边界情况：对从未保存的槽位调用 delete 应返回 EmptySlot 错误。
#[test]
fn test_delete_empty_slot_returns_error() {
    let (manager, _dir) = setup_manager();

    let result = manager.delete_save(99);
    assert!(
        matches!(result, Err(SaveError::EmptySlot { .. })),
        "删除空槽位应返回 EmptySlot，实际 {:?}",
        result
    );
}

/// 验证 SaveData 空状态往返 —— 使用 SaveData::new() 的最小默认状态。
///
/// 确保默认空存档也能正确保存和加载，不会因字段缺失而失败。
#[test]
fn test_save_load_empty_state() {
    let (manager, _dir) = setup_manager();

    let original = SaveData::new(0, "empty_scene");
    manager.save(0, &original).expect("保存应成功");

    let loaded = manager.load(0).expect("加载应成功");

    assert_eq!(loaded.version, SaveData::CURRENT_VERSION);
    assert_eq!(loaded.slot, 0);
    assert_eq!(loaded.scene_id, "empty_scene");
    assert!(loaded.variables.is_empty());
    assert!(loaded.flags.is_empty());
    assert!(loaded.label.is_none());
    assert!(loaded.audio_state.current_bgm_path.is_none());
    assert!(loaded.render_state.current_bg.is_none());
    assert!(loaded.render_state.displayed_sprites.is_empty());
    assert_eq!(loaded.vm_snapshot.pc, 0);
    assert!(loaded.vm_snapshot.call_stack.is_empty());
    assert!(loaded.vm_snapshot.stack.is_empty());
}

/// 验证 slot_exists 方法在文件存在/不存在时的正确性。
#[test]
fn test_slot_exists() {
    let (manager, _dir) = setup_manager();

    // 初始状态：所有槽位不存在
    assert!(!manager.slot_exists(0));
    assert!(!manager.slot_exists(QUICK_SLOT));
    assert!(!manager.slot_exists(99));

    // 保存后：槽位存在
    let data = full_save_data(0, "test");
    manager.save(0, &data).expect("保存应成功");
    assert!(manager.slot_exists(0));

    // 删除后：槽位不存在
    manager.delete_save(0).expect("删除应成功");
    assert!(!manager.slot_exists(0));
}

/// 验证不兼容的存档版本被正确检测。
///
/// 手动构造一个 version=99 的 SaveData，绕过正常的 save() 方法
/// 直接写入 MessagePack + CRC32 到文件，验证 load() 返回
/// `SaveError::IncompatibleVersion`。
#[test]
fn test_incompatible_save_version() {
    let (manager, _dir) = setup_manager();

    // 构造 version=99 的 SaveData
    let mut data = SaveData::new(0, "test_scene");
    data.version = 99;

    // 手动序列化并写入文件（绕过 save() 的版本检查）
    let msgpack_bytes = rmp_serde::to_vec(&data).expect("序列化应成功");
    let crc = crc32fast::hash(&msgpack_bytes);

    let mut file = fs::File::create(manager.slot_path(0)).expect("创建文件应成功");
    file.write_all(&crc.to_le_bytes())
        .expect("写入 CRC32 应成功");
    file.write_all(&msgpack_bytes).expect("写入数据应成功");
    file.flush().expect("flush 应成功");

    // 加载应检测到不兼容版本
    match manager.load(0) {
        Err(SaveError::IncompatibleVersion {
            found,
            expected,
            hint: _,
        }) => {
            assert_eq!(found, 99);
            assert_eq!(expected, 1); // CURRENT_VERSION
        }
        other => panic!("期望 IncompatibleVersion 错误，实际 {:?}", other),
    }
}
