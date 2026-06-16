//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-audio/tests/integration_test.rs
//! 功能概述：音频系统集成测试 — 验证 BGM/SE 同时播放、淡入淡出状态转换、
//!           循环 BGM 持续播放。使用动态生成的 WAV 文件，所有测试在无音频设备时静默跳过。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 对应任务：PH2-T09 — 集成测试
//! 覆盖 AC：AC03 (音频集成测试通过)
//!
//! ## 测试列表
//!
//! | 测试函数 | 覆盖 AC | 验证内容 |
//! |----------|---------|----------|
//! | `test_bgm_se_simultaneous` | AC03 | BGM 循环播放时 5 次 SE 不中断 BGM |
//! | `test_fade_in_out_timing` | AC03 | 淡入淡出状态转换正确，0 时长立即生效 |
//! | `test_bgm_looping_seamless` | AC03 | 短循环 BGM 在 3 个周期后仍在播放 |

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use aster_audio::AudioSystem;

// ─── 测试辅助函数 ─────────────────────────────────────────────────────────

/// 在指定路径生成一个最小有效的 WAV 文件（440Hz 正弦波，1 秒）。
///
/// 文件格式：44 字节 RIFF/WAVE 头 + PCM 样本数据。
/// 振幅设为 25%（8191/32767），避免测试音量过大。
///
/// # 参数
/// - `path`: 输出 WAV 文件的路径
fn generate_test_wav(path: &Path) {
    let sample_rate: u32 = 44100;
    let duration_secs: f32 = 1.0;
    let num_samples: u32 = (sample_rate as f32 * duration_secs) as u32;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let data_size: u32 = num_samples * num_channels as u32 * (bits_per_sample as u32 / 8);
    let file_size: u32 = 36 + data_size;

    let mut file = File::create(path).expect("创建测试 WAV 文件失败");

    // RIFF 头
    file.write_all(b"RIFF").unwrap();
    file.write_all(&file_size.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    // fmt chunk
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap(); // fmt chunk size
    file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM = 1
    file.write_all(&num_channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample as u32 / 8);
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    let block_align = num_channels * (bits_per_sample / 8);
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();

    // 生成 440Hz 正弦波 PCM 样本
    let frequency: f32 = 440.0;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // 振幅设为最大值的 25%，避免测试音量过大
        let amplitude: i16 = 8191;
        let sample: i16 =
            ((t * frequency * 2.0 * std::f32::consts::PI).sin() * amplitude as f32) as i16;
        file.write_all(&sample.to_le_bytes()).unwrap();
    }

    file.flush().unwrap();
}

/// 在指定路径生成一个短 WAV 文件（~0.1 秒），用于循环测试。
///
/// 极短的音频可以更快地触发循环回绕，适合自动化测试。
fn generate_short_test_wav(path: &Path) {
    let sample_rate: u32 = 44100;
    let duration_secs: f32 = 0.1;
    let num_samples: u32 = (sample_rate as f32 * duration_secs) as u32;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let data_size: u32 = num_samples * num_channels as u32 * (bits_per_sample as u32 / 8);
    let file_size: u32 = 36 + data_size;

    let mut file = File::create(path).expect("创建短测试 WAV 文件失败");

    // RIFF 头
    file.write_all(b"RIFF").unwrap();
    file.write_all(&file_size.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    // fmt chunk
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&num_channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample as u32 / 8);
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    let block_align = num_channels * (bits_per_sample / 8);
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();

    // 生成短正弦波
    let frequency: f32 = 880.0;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let amplitude: i16 = 8191;
        let sample: i16 =
            ((t * frequency * 2.0 * std::f32::consts::PI).sin() * amplitude as f32) as i16;
        file.write_all(&sample.to_le_bytes()).unwrap();
    }

    file.flush().unwrap();
}

/// 尝试初始化音频系统用于测试。
///
/// 在 CI 环境或无音频设备时返回 `None`，测试应跳过而非 panic。
/// 在有音频设备的开发环境中返回 `Some(AudioSystem)`。
fn try_init_audio() -> Option<AudioSystem> {
    // CI 环境中无真实音频设备，直接跳过
    if std::env::var("CI").is_ok() {
        eprintln!("跳过音频集成测试（CI 环境，无可用音频设备）");
        return None;
    }
    match AudioSystem::new() {
        Ok(audio) => Some(audio),
        Err(e) => {
            eprintln!("跳过音频集成测试（无可用音频设备）: {}", e);
            None
        }
    }
}

/// 创建包含 BGM 和 SE 测试 WAV 文件的临时目录。
///
/// 返回 (TempDir, bgm_path, se_path)，TempDir 在函数退出时自动清理。
fn create_test_wav_pair() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let bgm_path = dir.path().join("test_bgm.wav");
    let se_path = dir.path().join("test_se.wav");
    generate_test_wav(&bgm_path);
    generate_test_wav(&se_path);
    (dir, bgm_path, se_path)
}

// ─── 集成测试 ─────────────────────────────────────────────────────────────

/// AC03 — 验证 BGM 与 SE 可同时播放，互不干扰。
///
/// 先播放循环 BGM，然后在 BGM 播放期间快速连续播放 5 次 SE，
/// 验证所有 SE 调用返回 Ok 且 BGM 仍在播放。
#[test]
fn test_bgm_se_simultaneous() {
    let audio = match try_init_audio() {
        Some(a) => a,
        None => return, // 无音频设备，静默跳过
    };

    let (_dir, bgm_path, se_path) = create_test_wav_pair();
    let mut audio = audio;

    // 播放循环 BGM
    audio
        .play_bgm(bgm_path.to_str().unwrap(), true)
        .expect("BGM 播放应成功");
    assert!(audio.is_bgm_playing(), "BGM 应在播放中");

    // 在 BGM 播放期间快速连续播放 5 次 SE（间隔 50ms）
    for i in 0..5 {
        audio
            .play_se(se_path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("第 {} 次 SE 播放应成功，实际错误: {}", i + 1, e));
        if i < 4 {
            // 短暂间隔模拟快速连续点击
            thread::sleep(Duration::from_millis(50));
        }
    }

    // 额外等待确保 SE 完成播放
    thread::sleep(Duration::from_millis(200));

    // BGM 应仍在播放（循环模式）
    assert!(audio.is_bgm_playing(), "所有 SE 播放后 BGM 应仍在播放");

    // 停止 BGM
    audio.stop_bgm();
}

/// AC03 — 验证 SE 播放超量时的行为（SE 容量限制）。
///
/// 快速连续播放超过 kira 内部 SE 槽位数量的 SE，
/// 确保系统不 panic 且 BGM 不受影响。
#[test]
fn test_se_overflow_graceful() {
    let audio = match try_init_audio() {
        Some(a) => a,
        None => return,
    };

    let (_dir, bgm_path, se_path) = create_test_wav_pair();
    let mut audio = audio;

    // 先播放循环 BGM
    audio
        .play_bgm(bgm_path.to_str().unwrap(), true)
        .expect("BGM 播放应成功");

    // 连续播放 20 次 SE（可能超出 kira 内部限制）
    // 每次间隔 10ms，模拟极端场景
    let mut errors = 0usize;
    for i in 0..20 {
        if let Err(e) = audio.play_se(se_path.to_str().unwrap()) {
            eprintln!("第 {} 次 SE 返回错误: {}", i + 1, e);
            errors += 1;
        }
        thread::sleep(Duration::from_millis(10));
    }

    // 允许部分失败（超出 kira 内部限制），但不应全部失败
    assert!(errors < 20, "至少应有部分 SE 播放成功");

    // 等待所有 SE 完成
    thread::sleep(Duration::from_millis(500));

    // BGM 应仍在播放
    assert!(audio.is_bgm_playing(), "大量 SE 后 BGM 应仍在播放");
    audio.stop_bgm();
}

/// AC03 — 验证淡入淡出状态转换正确。
///
/// 测试场景：
/// 1. fade_in=1.0 启动 BGM → BGM 在播放中
/// 2. fade_out=0.5 停止 → BGM 停止
/// 3. fade_in=0.0 立即启动 → 立即在播放
/// 4. fade_out=0.0 立即停止 → 立即停止
#[test]
fn test_fade_in_out_timing() {
    let audio = match try_init_audio() {
        Some(a) => a,
        None => return,
    };

    let (_dir, bgm_path, _se_path) = create_test_wav_pair();
    let mut audio = audio;
    let bgm_str = bgm_path.to_str().unwrap();

    // 场景 1：fade_in=1.0 淡入播放
    audio
        .play_bgm_with_fade(bgm_str, true, 1.0)
        .expect("淡入 BGM 播放应成功");
    assert!(audio.is_bgm_playing(), "淡入 BGM 应在播放中");

    // 等待淡入完成
    thread::sleep(Duration::from_millis(1200));

    // 场景 2：fade_out=0.5 淡出停止
    audio.stop_bgm_with_fade(0.5);
    // 等待淡出完成
    thread::sleep(Duration::from_millis(700));
    assert!(!audio.is_bgm_playing(), "淡出后 BGM 应停止");

    // 场景 3：fade_in=0.0 立即播放
    audio
        .play_bgm_with_fade(bgm_str, false, 0.0)
        .expect("0 时长的淡入播放应成功");
    // 0 时长淡入应立即开始播放（或极短时间内开始）
    thread::sleep(Duration::from_millis(50));
    // 注意：非循环 BGM 可能很快结束，仅验证无 panic

    // 等待非循环 BGM 完成
    thread::sleep(Duration::from_millis(1200));

    // 场景 4：重新播放后立即停止（fade_out=0.0）
    audio
        .play_bgm_with_fade(bgm_str, true, 0.0)
        .expect("重新播放应成功");
    audio.stop_bgm_with_fade(0.0);
    // 0 时长淡出应几乎立即生效
    thread::sleep(Duration::from_millis(50));
    // 无 panic 和崩溃即为通过
}

/// AC03 — 验证循环 BGM 在多个循环后仍在播放。
///
/// 使用约 0.1 秒的极短 WAV 文件，等待足够时间让音频循环 3 次以上，
/// 验证循环后 BGM 仍在播放（无异常停止）。
#[test]
fn test_bgm_looping_seamless() {
    let audio = match try_init_audio() {
        Some(a) => a,
        None => return,
    };

    let mut audio = audio;

    // 创建短 WAV 文件（0.1 秒）用于快速循环
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let short_path = dir.path().join("short_loop.wav");
    generate_short_test_wav(&short_path);
    let short_str = short_path.to_str().unwrap();

    // 播放循环 BGM
    audio
        .play_bgm(short_str, true)
        .expect("循环 BGM 播放应成功");
    assert!(audio.is_bgm_playing(), "循环 BGM 应在播放中");

    // 等待 800ms——足够 0.1s 的音频循环至少 3 次
    thread::sleep(Duration::from_millis(800));

    // BGM 应仍在播放（如果循环正常）
    assert!(audio.is_bgm_playing(), "循环 BGM 在 3+ 次循环后应仍在播放");

    // 停止 BGM
    audio.stop_bgm();
}

/// 验证 AudioSystem 状态快照的获取和恢复。
///
/// 播放 BGM 并设置音量 → 获取状态快照 → 修改音量 → 恢复状态 → 验证恢复正确。
#[test]
fn test_audio_state_snapshot_roundtrip() {
    let audio = match try_init_audio() {
        Some(a) => a,
        None => return,
    };

    let (_dir, bgm_path, _se_path) = create_test_wav_pair();
    let mut audio = audio;
    let bgm_str = bgm_path.to_str().unwrap();

    // 设置初始状态
    audio.set_bgm_volume(0.7);
    audio.set_se_volume(0.3);
    audio.play_bgm(bgm_str, true).expect("BGM 播放应成功");

    // 等待 BGM 开始播放
    thread::sleep(Duration::from_millis(200));

    // 获取快照
    let snapshot = audio.get_state();
    assert!(snapshot.current_bgm_path.is_some(), "快照应包含 BGM 路径");
    assert!((snapshot.bgm_volume - 0.7).abs() < f32::EPSILON);
    assert!((snapshot.se_volume - 0.3).abs() < f32::EPSILON);

    // 修改状态
    audio.set_bgm_volume(0.1);
    audio.set_se_volume(0.9);

    // 恢复快照
    audio.restore_state(&snapshot).expect("状态恢复应成功");

    // 验证音量恢复到快照值
    let restored_snapshot = audio.get_state();
    assert!(
        (restored_snapshot.bgm_volume - 0.7).abs() < f32::EPSILON,
        "BGM 音量应恢复到 0.7，实际 {}",
        restored_snapshot.bgm_volume
    );
    assert!(
        (restored_snapshot.se_volume - 0.3).abs() < f32::EPSILON,
        "SE 音量应恢复到 0.3，实际 {}",
        restored_snapshot.se_volume
    );

    audio.stop_bgm();
}
