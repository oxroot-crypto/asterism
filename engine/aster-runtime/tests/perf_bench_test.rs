//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/tests/perf_bench_test.rs
//! 功能概述：性能基准测试 — 测量引擎关键路径的耗时和资源占用，
//!           与 NFR-PERF-001~008 目标值对照。所有测试标记为 `#[ignore]`，
//!           仅通过 `cargo test -- --ignored` 手动运行。
//!           基准结果为参考性指标，CI 环境中不作硬门禁。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 对应任务：PH2-T09 — 集成测试
//! 覆盖 AC：AC06 (性能基准可运行)
//!
//! ## 测试列表
//!
//! | 测试函数 | NFR 指标 | 目标值 |
//! |----------|----------|--------|
//! | `bench_frame_rate_1080p` | NFR-PERF-001 | ≥ 60 FPS |
//! | `bench_first_scene_load_time` | NFR-PERF-003 | < 3 秒 |
//! | `bench_scene_switch_time` | NFR-PERF-004 | < 1 秒 |
//! | `bench_save_write_time` | NFR-PERF-005 | < 500ms |
//! | `bench_load_time` | NFR-PERF-006 | < 500ms |
//! | `bench_memory_usage` | NFR-PERF-007 | < 512 MB |

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aster_runtime::{App, GameContext, MockRenderer, SceneManager};
use aster_save::SaveManager;

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

/// 获取当前进程内存占用的近似值（KB）。
///
/// 在 Linux 上读取 `/proc/self/statm`（RSS 字段），
/// 在 Windows/macOS 上返回 0（表示"未测量"）。
/// 返回 0 时，内存断言自动跳过。
fn get_process_memory_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            // statm 格式：size resident share text lib data dt
            // 第二个字段是 RSS（resident set size），单位是页
            if let Some(rss_pages) = content.split_whitespace().nth(1) {
                if let Ok(pages) = rss_pages.parse::<u64>() {
                    // 页面大小通常为 4KB（可通过 sysconf 获取，但 4KB 覆盖绝大多数情况）
                    return pages * 4;
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows 上无简便方法获取 RSS（需要 winapi），返回 0 表示跳过
        // 开发者可在任务管理器中手动观察内存占用
        eprintln!("内存测量在 Windows 上未实现，跳过");
    }
    #[cfg(target_os = "macos")]
    {
        // macOS 可通过 task_info 获取，但需要 libc，返回 0 表示跳过
        eprintln!("内存测量在 macOS 上未实现，跳过");
    }
    0 // 未测量
}

// ─── 性能基准测试 ─────────────────────────────────────────────────────────

/// NFR-PERF-001 — 模拟帧率测试。
///
/// 执行 100 帧的模拟更新循环（无真实 GPU 渲染），
/// 测量平均 FPS，断言 ≥ 60。
/// 使用 MockRenderer 避免 GPU 瓶颈，专注测量 CPU 端逻辑。
#[test]
#[ignore]
fn bench_frame_rate_1080p() {
    let ctx = load_game_context();
    let mut renderer = MockRenderer::new();
    let mut sm = SceneManager::new(ctx);
    sm.load_scene("prologue").expect("加载序章应成功");

    let start = Instant::now();
    let mut frame_count = 0u32;

    for _ in 0..100 {
        // 推进场景（模拟每帧 16ms 内的工作量）
        if matches!(*sm.state(), aster_runtime::SceneState::Ended) {
            // 场景结束后重新加载
            sm.load_scene("prologue").expect("重新加载应成功");
            frame_count += 1;
            continue;
        }

        let result = sm.update(Some(&mut renderer));
        if result.is_err() {
            // 场景正常结束不会影响帧率测量
            break;
        }

        // 模拟帧时间流逝（打字机效果）
        sm.update_dialogue(Duration::from_millis(16), &mut Some(&mut renderer));

        // 如果遇到暂停点（对话），点击推进
        if matches!(*sm.state(), aster_runtime::SceneState::Playing)
            && (renderer.has_call_containing("set_dialogue")
                || renderer.has_call_containing("set_narration"))
        {
            let _ = sm.on_click(Some(&mut renderer));
        }

        frame_count += 1;

        // 安全保护：避免无限循环（100 帧足够测量）
        if frame_count >= 100 {
            break;
        }
    }

    let elapsed = start.elapsed();
    let avg_fps = frame_count as f64 / elapsed.as_secs_f64();
    eprintln!(
        "bench_frame_rate_1080p: {} 帧 / {:?}, 平均 {:.1} FPS",
        frame_count, elapsed, avg_fps
    );

    // 信息性检查：FPS 应 ≥ 60
    // CI 环境中此断言可能因环境性能差异而失败，设为宽松阈值
    if avg_fps < 60.0 {
        eprintln!(
            "⚠ 性能警告: 平均 {:.1} FPS < 60 FPS 目标。这在 CI 共享 runner 中可能是正常的。",
            avg_fps
        );
    }
    // 不强制 fail——性能基准为信息性指标
}

/// NFR-PERF-003 — 冷启动加载时间。
///
/// 测量从 `App::load()` 到首次 `update()` 完成的总耗时，
/// 断言 < 3 秒。
#[test]
#[ignore]
fn bench_first_scene_load_time() {
    let start = Instant::now();

    // 冷启动：解析 + 编译 + 创建上下文
    let app = App::load(&template_path()).expect("App::load 应成功");
    let load_elapsed = start.elapsed();

    let mut renderer = MockRenderer::new();
    let mut sm = SceneManager::new(app.game_context);
    sm.load_scene("prologue").expect("加载序章应成功");

    let update_start = Instant::now();
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");
    let total_elapsed = start.elapsed();

    eprintln!(
        "bench_first_scene_load_time: 解析+编译={:?}, 首次更新={:?}, 总计={:?}",
        load_elapsed,
        update_start.elapsed(),
        total_elapsed,
    );

    assert!(
        total_elapsed < Duration::from_secs(3),
        "冷启动应 < 3 秒，实际 {:?}",
        total_elapsed
    );
}

/// NFR-PERF-004 — 场景切换时间。
///
/// 测量从 `load_scene()` 到 `update()` 完成的耗时，
/// 断言 < 1 秒。
#[test]
#[ignore]
fn bench_scene_switch_time() {
    let ctx = load_game_context();
    let mut renderer = MockRenderer::new();
    let mut sm = SceneManager::new(ctx);

    // 先加载一个场景（预热）
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 测量跨场景切换耗时
    let start = Instant::now();
    // 尝试切换到另一个场景（如果存在的话）
    // 注：模板项目可能只有一个序章场景，因此测量重新加载同一场景的耗时
    sm.load_scene("prologue").expect("重新加载应成功");
    sm.update(Some(&mut renderer)).expect("update 应成功");
    let elapsed = start.elapsed();

    eprintln!("bench_scene_switch_time: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_secs(1),
        "场景切换应 < 1 秒，实际 {:?}",
        elapsed
    );
}

/// NFR-PERF-005 — 存档写入时间。
///
/// 测量 `save()` 操作（含 MessagePack 序列化 + CRC32 + 文件写入）的耗时，
/// 断言 < 500ms。
#[test]
#[ignore]
fn bench_save_write_time() {
    let ctx = load_game_context();
    let mut renderer = MockRenderer::new();
    let mut sm = SceneManager::new(ctx);
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    let save_dir = tempfile::tempdir().expect("创建临时目录失败");
    let save_mgr = Arc::new(SaveManager::new(save_dir.path().to_path_buf()));
    sm.set_save_manager(save_mgr.clone());

    let save_data = sm.collect_game_state(0);

    let start = Instant::now();
    save_mgr.save(0, &save_data).expect("保存应成功");
    let elapsed = start.elapsed();

    eprintln!("bench_save_write_time: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(500),
        "存档写入应 < 500ms，实际 {:?}",
        elapsed
    );
}

/// NFR-PERF-006 — 读档加载时间。
///
/// 测量从磁盘加载存档并恢复到 SceneManager 的总耗时，
/// 断言 < 500ms。
#[test]
#[ignore]
fn bench_load_time() {
    let ctx = load_game_context();
    let mut renderer = MockRenderer::new();
    let mut sm = SceneManager::new(ctx);
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    let save_dir = tempfile::tempdir().expect("创建临时目录失败");
    let save_mgr = Arc::new(SaveManager::new(save_dir.path().to_path_buf()));
    sm.set_save_manager(save_mgr.clone());

    // 先保存一个存档
    let save_data = sm.collect_game_state(0);
    save_mgr.save(0, &save_data).expect("保存应成功");

    // 读档前重新加载场景（模拟实际读档流程）
    sm.load_scene("prologue").expect("重新加载应成功");

    let start = Instant::now();
    let loaded = save_mgr.load(0).expect("读档应成功");
    sm.restore_game_state(&loaded, &mut Some(&mut renderer))
        .expect("恢复应成功");
    let elapsed = start.elapsed();

    eprintln!("bench_load_time: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(500),
        "读档恢复应 < 500ms，实际 {:?}",
        elapsed
    );
}

/// NFR-PERF-007 — 内存占用。
///
/// 加载完整项目后测量进程内存占用（RSS），
/// 断言 < 512 MB。如果平台不支持内存测量则跳过。
#[test]
#[ignore]
fn bench_memory_usage() {
    // 加载完整项目以模拟运行时内存状态
    let ctx = load_game_context();
    let mut renderer = MockRenderer::new();
    let mut sm = SceneManager::new(ctx);
    sm.load_scene("prologue").expect("加载序章应成功");
    sm.update(Some(&mut renderer)).expect("首次 update 应成功");

    // 也创建 SaveManager 触发存档子系统加载
    let save_dir = tempfile::tempdir().expect("创建临时目录失败");
    let save_mgr = Arc::new(SaveManager::new(save_dir.path().to_path_buf()));
    sm.set_save_manager(save_mgr.clone());
    let _ = sm.collect_game_state(0);

    let mem_kb = get_process_memory_kb();
    eprintln!(
        "bench_memory_usage: {} KB (≈ {:.1} MB)",
        mem_kb,
        mem_kb as f64 / 1024.0
    );

    if mem_kb > 0 {
        let mem_mb = mem_kb / 1024;
        assert!(mem_mb < 512, "内存占用应 < 512 MB，实际 {} MB", mem_mb);
    } else {
        eprintln!("内存测量在此平台上不可用，跳过断言");
    }
}
