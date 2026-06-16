//! 生成测试用音频文件到模板项目 assets/ 目录
//!
//! 运行: cargo run --package aster-audio --example gen_test_audio

use std::fs;
use std::path::Path;

fn main() {
    let template_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project/assets");

    let bgm_dir = template_dir.join("bgm");
    let se_dir = template_dir.join("se");
    fs::create_dir_all(&bgm_dir).expect("创建 bgm 目录失败");
    fs::create_dir_all(&se_dir).expect("创建 se 目录失败");

    // 场景中引用的 BGM 列表
    let bgm_list = &[
        "bgm_daily_life",
        "bgm_tender_moment",
        "bgm_title_theme",
        "bgm_memory",
        "bgm_sakura_road",
        "bgm_quiet_library",
    ];

    // 场景中引用的 SE 列表
    let se_list = &[
        "se_birds_chirping",
        "se_rain_ambient",
        "se_bell_ring",
        "se_door_open",
        "se_memory_chime",
        "se_mystery_chime",
    ];

    for name in bgm_list {
        let path = bgm_dir.join(format!("{}.wav", name));
        make_bgm(&path, name);
        println!("  生成 BGM: {}", path.display());
    }

    for name in se_list {
        let path = se_dir.join(format!("{}.wav", name));
        make_se(&path, name);
        println!("  生成 SE: {}", path.display());
    }

    println!("\n所有测试音频文件已生成到: {}", template_dir.display());
}

/// 根据名称生成不同音调的 BGM（30 秒，简单的音阶循环）
fn make_bgm(path: &Path, name: &str) {
    let freq = match name {
        "bgm_daily_life" => 261.63,    // C4 — 日常活泼
        "bgm_tender_moment" => 329.63, // E4 — 温柔
        "bgm_title_theme" => 392.00,   // G4 — 标题画面
        "bgm_memory" => 293.66,        // D4 — 回忆
        "bgm_sakura_road" => 349.23,   // F4 — 樱花道
        "bgm_quiet_library" => 220.00, // A3 — 安静图书馆
        _ => 261.63,
    };
    let sample_rate = 44100u32;
    let duration = 30.0f32;
    let num_samples = (sample_rate as f32 * duration) as usize;
    let samples: Vec<i16> = (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            // 简单的音调 + 泛音
            let fundamental = (t * freq * 2.0 * std::f32::consts::PI).sin();
            let overtone = (t * freq * 1.5 * 2.0 * std::f32::consts::PI).sin() * 0.3;
            let vibrato = (t * 4.0 * 2.0 * std::f32::consts::PI).sin() * 0.05;
            let envelope = 1.0 - (t / duration).min(1.0) * 0.2;
            let sample = (fundamental + overtone + vibrato) * envelope * 0.5;
            (sample * 32767.0) as i16
        })
        .collect();

    write_wav(path, &samples, sample_rate, 1);
}

/// 生成短音效（0.5-3 秒，不同特征）
fn make_se(path: &Path, name: &str) {
    let sample_rate = 44100u32;
    let (duration, freq, kind) = match name {
        "se_birds_chirping" => (1.5f32, 1200.0f32, "chirp"),
        "se_rain_ambient" => (3.0f32, 200.0f32, "noise"),
        "se_bell_ring" => (2.0f32, 880.0f32, "bell"),
        "se_door_open" => (1.0f32, 300.0f32, "sweep"),
        "se_memory_chime" => (2.0f32, 523.25f32, "chime"),
        "se_mystery_chime" => (2.5f32, 440.0f32, "chime"),
        _ => (1.0, 440.0, "chime"),
    };
    let num_samples = (sample_rate as f32 * duration) as usize;
    let samples: Vec<i16> = (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = match kind {
                "chirp" => {
                    let f = 800.0 + (t / duration) * 1600.0;
                    (t * f * 2.0 * std::f32::consts::PI).sin() * (1.0 - t / duration)
                }
                "noise" => {
                    // 伪随机噪声模拟雨声
                    let noise =
                        (i as f32 * 12_345.68).sin() * 0.5 + (i as f32 * 9_876.543).sin() * 0.3;
                    noise * (1.0 - t / duration) * 0.3
                }
                "bell" => {
                    let envelope = (-t * 3.0).exp();
                    (t * freq * 2.0 * std::f32::consts::PI).sin() * envelope
                        + (t * freq * 2.07 * 2.0 * std::f32::consts::PI).sin() * envelope * 0.5
                }
                "sweep" => {
                    let f = 100.0 + (t / duration) * 600.0;
                    (t * f * 2.0 * std::f32::consts::PI).sin() * (1.0 - t / duration) * 0.5
                }
                "chime" => {
                    let envelope = (-t * 2.0).exp();
                    (t * freq * 2.0 * std::f32::consts::PI).sin() * envelope * 0.6
                        + (t * freq * 1.5 * 2.0 * std::f32::consts::PI).sin() * envelope * 0.3
                }
                _ => 0.0,
            };
            (sample * 32767.0) as i16
        })
        .collect();

    write_wav(path, &samples, sample_rate, 1);
}

/// 写入最小 WAV 文件
fn write_wav(path: &Path, i16_samples: &[i16], sample_rate: u32, channels: u16) {
    let data_size = (i16_samples.len() * 2) as u32;
    let file_size = 44 + data_size;
    let mut buf = Vec::with_capacity(file_size as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    let byte_rate = sample_rate * channels as u32 * 2;
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = channels * 2;
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes()); // 16-bit

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &sample in i16_samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }

    fs::write(path, buf).expect("写入 WAV 文件失败");
}
