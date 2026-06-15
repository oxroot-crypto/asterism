//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-audio/src/audio_system.rs
//! 功能概述：音频系统核心 — `AudioSystem` 结构体封装 kira 音频管理器，
//!           提供 BGM 背景音乐和 SE 音效的加载、播放、停止、循环和音量控制。
//!           本模块是音频系统的基石，后续 fade（PH2-T03）在此基础上扩展。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - kira（音频引擎后端：AudioManager / TrackBuilder / TrackHandle / StaticSoundData）
//! - crate::error::AudioError（错误类型）

use std::path::Path;

use crate::error::AudioError;

/// 将振幅比值（0.0 ~ 1.0）转换为分贝值。
///
/// 换算公式：`dB = 20 × log₁₀(amplitude)`
///
/// # 特殊值
/// - `1.0` → `Decibels::IDENTITY`（0 dB，原始音量）
/// - `≤ 0.0` → `Decibels::SILENCE`（-60 dB，静音）
///
/// kira 0.12 使用 `Decibels` 而非振幅比值控制音量，
/// 此函数提供从用户友好的 0.0~1.0 到 dB 的转换。
fn amplitude_to_db(ratio: f32) -> kira::Decibels {
    if ratio <= 0.0 {
        kira::Decibels::SILENCE
    } else if ratio >= 1.0 {
        kira::Decibels::IDENTITY
    } else {
        // 20 * log10(ratio)
        kira::Decibels(20.0 * ratio.log10())
    }
}

/// 音频系统 — 管理 BGM/SE 的播放、停止和音量控制。
///
/// 封装 kira 音频引擎，BGM 和 SE 通过独立子轨道（TrackHandle）隔离混音，
/// 互不干扰。AudioSystem 设计为具体结构体而非 trait——当前无多后端需求，
/// 且与 `aster-renderer` 的 `Renderer` trait 设计模式不同。
///
/// # 音频通道架构
///
/// ```text
/// AudioManager (main track)
///   ├── bgm_track → BGM 播放（支持循环、单曲独占）
///   └── se_track  → SE 播放（fire-and-forget、支持并发）
/// ```
///
/// # 线程安全
///
/// `kira::AudioManager` 内部使用 `Arc<Mutex<>>` 管理音频资源，
/// 因此 `AudioSystem` 自动满足 `Send + Sync`，可在多线程环境
/// （如 SceneManager）中安全持有。
///
/// # 字段
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `manager` | `kira::AudioManager<DefaultBackend>` | kira 音频管理器，持有音频设备和混音图 |
/// | `bgm_track` | `kira::track::TrackHandle` | BGM 子轨道，独立音量控制 |
/// | `se_track` | `kira::track::TrackHandle` | SE 子轨道，独立音量控制 |
/// | `bgm_handle` | `Option<StaticSoundHandle>` | 当前 BGM 播放句柄，用于停止 |
/// | `current_bgm_path` | `Option<String>` | 当前播放的 BGM 文件路径（用于状态快照 PH2-T03） |
/// | `bgm_volume` | `f32` | BGM 通道当前音量（0.0 ~ 1.0） |
/// | `se_volume` | `f32` | SE 通道当前音量（0.0 ~ 1.0） |
///
/// # 示例
///
/// ```rust,no_run
/// use aster_audio::AudioSystem;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut audio = AudioSystem::new()?;
/// audio.play_bgm("assets/bgm/theme.ogg", true)?;
/// audio.play_se("assets/se/click.wav")?;
/// audio.set_bgm_volume(0.8);
/// audio.set_se_volume(0.6);
/// assert!(audio.is_bgm_playing());
/// audio.stop_bgm();
/// # Ok(())
/// # }
/// ```
pub struct AudioSystem {
    /// kira 音频管理器 — 持有音频设备连接和混音图
    /// 使用 DefaultBackend（cpal），自动选择平台原生音频驱动
    /// 注意：manager 不直接调用 play()，但持有 main track 所有权，
    /// bgm_track/se_track 依赖其生命周期
    #[allow(dead_code)]
    manager: kira::AudioManager<kira::DefaultBackend>,
    /// BGM 子轨道 — 独立音量控制，BGM 独占播放
    bgm_track: kira::track::TrackHandle,
    /// SE 子轨道 — 独立音量控制，支持多个 SE 并发播放
    se_track: kira::track::TrackHandle,
    /// 当前 BGM 播放句柄 — None 表示无 BGM 播放中
    /// 持有此句柄可以停止播放
    bgm_handle: Option<kira::sound::static_sound::StaticSoundHandle>,
    /// 当前 BGM 文件路径 — 为后续音频状态快照（PH2-T03）做准备
    current_bgm_path: Option<String>,
    /// BGM 通道音量（0.0 ~ 1.0），默认 0.8
    bgm_volume: f32,
    /// SE 通道音量（0.0 ~ 1.0），默认 0.8
    se_volume: f32,
}

impl AudioSystem {
    /// 创建并初始化音频系统。
    ///
    /// 初始化 kira 音频管理器（使用默认 AudioManagerSettings），
    /// 配置采样率 44100Hz，自动选择平台原生音频后端（Windows: WASAPI，
    /// macOS: CoreAudio，Linux: ALSA/PulseAudio）。
    ///
    /// # 错误
    ///
    /// - `AudioError::PlaybackError` — 系统音频设备不可用（无音频设备、
    ///   驱动异常、设备被独占等场景）
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// let audio = AudioSystem::new()
    ///     .expect("初始化音频系统失败，请检查音频设备是否可用");
    /// ```
    pub fn new() -> Result<Self, AudioError> {
        // 使用默认设置创建 kira 音频管理器
        // AudioManagerSettings::default() 配置：
        // - 采样率：44100 Hz
        // - 后端：DefaultBackend（cpal，跨平台音频库，自动选择平台原生驱动）
        let mut manager =
            kira::AudioManager::<kira::DefaultBackend>::new(kira::AudioManagerSettings::default())
                .map_err(|e| AudioError::PlaybackError {
                    reason: format!("无法创建音频管理器：{}", e),
                })?;

        // 创建 BGM 子轨道 — 单曲独占，容量 4 首
        let bgm_track = manager
            .add_sub_track(kira::track::TrackBuilder::new().sound_capacity(4))
            .map_err(|e| AudioError::PlaybackError {
                reason: format!("无法创建 BGM 子轨道：{}", e),
            })?;

        // 创建 SE 子轨道 — 支持并发音效，容量 16 首
        let se_track = manager
            .add_sub_track(kira::track::TrackBuilder::new().sound_capacity(16))
            .map_err(|e| AudioError::PlaybackError {
                reason: format!("无法创建 SE 子轨道：{}", e),
            })?;

        Ok(Self {
            manager,
            bgm_track,
            se_track,
            bgm_handle: None,
            current_bgm_path: None,
            // 默认 BGM 音量 0.8，与 GameSettings::default_bgm_volume 一致
            bgm_volume: 0.8,
            // 默认 SE 音量 0.8，与 GameSettings::default_se_volume 一致
            se_volume: 0.8,
        })
    }

    /// 加载音频文件为 kira 可播放的声音数据。
    ///
    /// BGM 和 SE 播放的共用入口，处理文件存在性检查和格式解码。
    /// 使用 symphonia 自动检测格式（OGG/FLAC/MP3/WAV）。
    ///
    /// # 参数
    /// - `asset_path`：音频文件路径
    ///
    /// # 错误
    /// - `AudioError::AssetNotFound` — 文件不存在
    /// - `AudioError::DecodeError` — 格式不支持或内容损坏
    fn load_sound_data(
        asset_path: &str,
    ) -> Result<kira::sound::static_sound::StaticSoundData, AudioError> {
        let path = Path::new(asset_path);

        // 检查文件是否存在
        if !path.exists() {
            return Err(AudioError::AssetNotFound {
                path: asset_path.to_string(),
            });
        }

        // 使用 kira/symphonia 自动检测格式并解码
        kira::sound::static_sound::StaticSoundData::from_file(path).map_err(|e| {
            AudioError::DecodeError {
                reason: format!("无法解码音频文件 \"{}\"：{}", asset_path, e),
            }
        })
    }

    /// 加载并播放背景音乐（BGM）。
    ///
    /// 通过 kira 加载指定路径的音频文件（支持 OGG/FLAC/MP3/WAV），
    /// 解码后在 BGM 通道上播放。如果当前已有 BGM 播放中（AC08），
    /// 则先停止旧 BGM 再播放新曲目，确保同一时间只有一首 BGM。
    ///
    /// # 参数
    ///
    /// - `asset_path`：音频文件路径，相对于项目根目录
    ///   （如 `"assets/bgm/theme.ogg"`）
    /// - `looping`：是否循环播放。`true` 时 BGM 播完后自动从头重新开始
    ///
    /// # 错误
    ///
    /// - `AudioError::AssetNotFound` — 指定路径的文件不存在
    /// - `AudioError::DecodeError` — 文件格式不支持或内容损坏
    /// - `AudioError::PlaybackError` — kira 播放提交失败
    ///
    /// # BGM 替换策略（AC08）
    ///
    /// 当已有 BGM 播放中时调用本方法，会先执行 `stop_bgm()` 停止
    /// 当前 BGM，然后加载并播放新 BGM。旧 BGM 的音频资源被立即释放。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut audio = AudioSystem::new()?;
    /// // 循环播放主题曲
    /// audio.play_bgm("assets/bgm/main_theme.ogg", true)?;
    /// // 切换到不循环的场景 BGM
    /// audio.play_bgm("assets/bgm/scene_01.ogg", false)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn play_bgm(&mut self, asset_path: &str, looping: bool) -> Result<(), AudioError> {
        // 如果已有 BGM 播放中，先停止（AC08：BGM 替换）
        self.stop_bgm();

        // 加载音频文件（BGM/SE 共用解码逻辑）
        let mut sound_data = Self::load_sound_data(asset_path)?;

        // 设置循环播放
        if looping {
            // 使用 kira 原生 loop_region 实现无缝循环
            // `..` 表示从开始到结束的完整区域（无限循环）
            sound_data = sound_data.loop_region(..);
        }

        // 提交播放到 BGM 子轨道，获取句柄
        let handle = self
            .bgm_track
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError {
                reason: format!("无法播放音频 \"{}\"：{}", asset_path, e),
            })?;

        // 保存播放状态
        self.bgm_handle = Some(handle);
        self.current_bgm_path = Some(asset_path.to_string());

        Ok(())
    }

    /// 停止当前播放的背景音乐。
    ///
    /// 立即停止 BGM 播放并释放音频资源。如果当前无 BGM 播放中，
    /// 则本方法为无操作（no-op），不产生任何效果。
    ///
    /// # 清理行为
    ///
    /// 调用后：
    /// - `bgm_handle` 被设为 `None`
    /// - `current_bgm_path` 被设为 `None`
    /// - 底层 kira 音频资源被释放
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut audio = AudioSystem::new()?;
    /// audio.play_bgm("assets/bgm/theme.ogg", true)?;
    /// // ... 游戏逻辑 ...
    /// audio.stop_bgm();
    /// assert!(!audio.is_bgm_playing());
    /// # Ok(())
    /// # }
    /// ```
    pub fn stop_bgm(&mut self) {
        if let Some(mut handle) = self.bgm_handle.take() {
            // 使用默认 Tween（立即停止，无淡出效果）
            // PH2-T03 将扩展为支持 fade_out 参数
            handle.stop(kira::Tween::default());
        }
        self.current_bgm_path = None;
    }

    /// 设置 BGM 通道音量。
    ///
    /// 音量值自动钳制到 `0.0 ~ 1.0` 范围：
    /// - `0.0` = 静音（mute），音频仍播放但不发声
    /// - `1.0` = 最大音量
    ///
    /// 音量更改即时生效，无过渡动画
    /// （PH2-T03 将通过 Tween 实现平滑音量过渡）。
    ///
    /// # 参数
    ///
    /// - `volume`：目标音量（0.0 ~ 1.0），超出范围自动钳制
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut audio = AudioSystem::new()?;
    /// audio.set_bgm_volume(0.5);
    /// assert!((audio.bgm_volume() - 0.5).abs() < f32::EPSILON);
    /// audio.set_bgm_volume(2.0);  // 钳制到 1.0
    /// assert!((audio.bgm_volume() - 1.0).abs() < f32::EPSILON);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_bgm_volume(&mut self, volume: f32) {
        // 钳制到 [0.0, 1.0] 范围
        let clamped = volume.clamp(0.0, 1.0);
        self.bgm_volume = clamped;

        // 通过 BGM 子轨道设置音量，所有通过此轨道的声音均受影响
        let db = amplitude_to_db(clamped);
        self.bgm_track.set_volume(db, kira::Tween::default());
    }

    /// 获取当前 BGM 通道音量。
    ///
    /// # 返回值
    ///
    /// 当前音量值（0.0 ~ 1.0）
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # let audio = AudioSystem::new().unwrap();
    /// assert!((audio.bgm_volume() - 0.8).abs() < f32::EPSILON);
    /// ```
    pub fn bgm_volume(&self) -> f32 {
        self.bgm_volume
    }

    /// 播放一次性音效（SE）。
    ///
    /// 在 SE 子轨道上播放指定路径的音频文件。SE 采用 fire-and-forget
    /// 模式——方法立即返回，不持有播放句柄，音频播完后自动释放资源。
    /// SE 可与 BGM 同时播放，互不干扰。
    ///
    /// # 参数
    ///
    /// - `asset_path`：音频文件路径（如 `"assets/se/click.wav"`）
    ///
    /// # 错误
    ///
    /// - `AudioError::AssetNotFound` — 指定路径的文件不存在
    /// - `AudioError::DecodeError` — 文件格式不支持或内容损坏
    /// - `AudioError::PlaybackError` — kira 播放提交失败
    ///
    /// # 并发播放
    ///
    /// SE 子轨道容量为 16，支持最多 16 个 SE 同时播放。
    /// 超出容量时 kira 内部排队，返回错误而非阻塞。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut audio = AudioSystem::new()?;
    /// audio.play_se("assets/se/click.wav")?;
    /// audio.play_se("assets/se/confirm.wav")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn play_se(&mut self, asset_path: &str) -> Result<(), AudioError> {
        // 加载音频文件（BGM/SE 共用解码逻辑）
        let sound_data = Self::load_sound_data(asset_path)?;

        // 提交播放到 SE 子轨道（fire-and-forget，不持有句柄）
        self.se_track
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError {
                reason: format!("无法播放音效 \"{}\"：{}", asset_path, e),
            })?;

        Ok(())
    }

    /// 设置 SE 通道音量。
    ///
    /// 通过 SE 子轨道独立控制音量，不影响 BGM 音量。
    /// 音量值自动钳制到 `0.0 ~ 1.0` 范围。
    ///
    /// # 参数
    ///
    /// - `volume`：目标音量（0.0 ~ 1.0），超出范围自动钳制
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut audio = AudioSystem::new()?;
    /// audio.set_se_volume(0.5);
    /// assert!((audio.se_volume() - 0.5).abs() < f32::EPSILON);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_se_volume(&mut self, volume: f32) {
        // 钳制到 [0.0, 1.0] 范围
        let clamped = volume.clamp(0.0, 1.0);
        self.se_volume = clamped;

        // 通过 SE 子轨道设置音量，所有通过此轨道的声音均受影响
        let db = amplitude_to_db(clamped);
        self.se_track.set_volume(db, kira::Tween::default());
    }

    /// 获取当前 SE 通道音量。
    ///
    /// # 返回值
    ///
    /// 当前 SE 音量值（0.0 ~ 1.0）
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # let audio = AudioSystem::new().unwrap();
    /// assert!((audio.se_volume() - 0.8).abs() < f32::EPSILON);
    /// ```
    pub fn se_volume(&self) -> f32 {
        self.se_volume
    }

    /// 检查是否有 BGM 正在播放。
    ///
    /// # 返回值
    ///
    /// - `true` — 当前有 BGM 在播放中
    /// - `false` — 当前无 BGM 播放（初始状态或已停止）
    ///
    /// # 注意
    ///
    /// 此方法仅检查 `bgm_handle` 是否为 `Some`，不查询 kira 内部
    /// 播放状态。对于大多数视觉小说场景，此检查足够准确。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use aster_audio::AudioSystem;
    /// # let mut audio = AudioSystem::new().unwrap();
    /// assert!(!audio.is_bgm_playing());
    /// ```
    pub fn is_bgm_playing(&self) -> bool {
        self.bgm_handle.is_some()
    }
}

// ─── 测试模块 ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 生成最小有效的 WAV 文件用于播放测试。
    ///
    /// 生成 1 秒 44100Hz 单声道 16-bit PCM 的 440Hz 正弦波 WAV 文件。
    /// WAV 格式简单且被 kira/symphonia 广泛支持，适合自动化测试。
    ///
    /// # WAV 文件结构（44 字节头 + 样本数据）
    ///
    /// | 偏移 | 大小 | 内容 |
    /// |------|------|------|
    /// | 0 | 4 | "RIFF" |
    /// | 4 | 4 | 文件大小 - 8 |
    /// | 8 | 4 | "WAVE" |
    /// | 12 | 4 | "fmt " |
    /// | 16 | 4 | fmt chunk 大小（16） |
    /// | 20 | 2 | 音频格式（1 = PCM） |
    /// | 22 | 2 | 声道数（1 = 单声道） |
    /// | 24 | 4 | 采样率（44100） |
    /// | 28 | 4 | 字节率（采样率 × 声道数 × 位深/8） |
    /// | 32 | 2 | 块对齐（声道数 × 位深/8） |
    /// | 34 | 2 | 位深（16） |
    /// | 36 | 4 | "data" |
    /// | 40 | 4 | 数据大小 |
    /// | 44 | N | PCM 样本数据 |
    fn generate_test_wav(path: &Path) {
        let sample_rate: u32 = 44100;
        let duration_secs: f32 = 1.0;
        let num_samples: u32 = (sample_rate as f32 * duration_secs) as u32;
        let num_channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let data_size: u32 = num_samples * num_channels as u32 * (bits_per_sample as u32 / 8);
        let file_size: u32 = 36 + data_size;

        let mut file = std::fs::File::create(path).expect("创建测试 WAV 文件失败");

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
            let amplitude: i16 = 8191; // ~25% of max 32767
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
    ///
    /// # CI 检测
    ///
    /// GitHub Actions 等 CI 运行器的 Windows Server 无音频硬件，
    /// `cpal` WASAPI 后端会触发 STATUS_ACCESS_VIOLATION（0xc0000005），
    /// 因此检测到 `CI` 环境变量时直接跳过，不尝试初始化。
    fn try_init_audio_system() -> Option<AudioSystem> {
        // CI 环境中无真实音频设备，直接跳过（避免 Windows 上 cpal 崩溃）
        if std::env::var("CI").is_ok() {
            eprintln!("跳过音频测试（CI 环境，无可用音频设备）");
            return None;
        }
        match AudioSystem::new() {
            Ok(audio) => Some(audio),
            Err(e) => {
                eprintln!("跳过音频测试（无可用音频设备）: {}", e);
                None
            }
        }
    }

    // ─── AC01: AudioSystem 初始化 ────────────────────────────────────────

    /// AC01 — AudioSystem 初始化成功。
    ///
    /// 验证 `AudioSystem::new()` 返回 `Ok`，各字段初始值正确。
    #[test]
    fn ac01_audio_system_init_success() {
        let audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        assert!(!audio.is_bgm_playing(), "初始状态应无 BGM 播放");
        assert!(
            (audio.bgm_volume() - 0.8).abs() < f32::EPSILON,
            "默认音量应为 0.8"
        );
    }

    // ─── AC02: BGM 播放 ────────────────────────────────────────────────

    /// AC02 — BGM 播放正常。
    ///
    /// 验证调用 `play_bgm` 后：
    /// 1. 返回 Ok
    /// 2. `is_bgm_playing()` 返回 true
    #[test]
    fn ac02_bgm_play_normal() {
        let temp_dir = std::env::temp_dir().join("aster_test_ac02");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_ac02.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        let result = audio.play_bgm(wav_path.to_str().unwrap(), false);

        assert!(result.is_ok(), "play_bgm 应返回 Ok，实际: {:?}", result);
        assert!(audio.is_bgm_playing(), "播放后 is_bgm_playing 应为 true");

        // 清理
        audio.stop_bgm();
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ─── AC03: BGM 无缝循环 ──────────────────────────────────────────────

    /// AC03 — BGM 无缝循环。
    ///
    /// 验证调用 `play_bgm(looping: true)` 后 BGM 正常播放。
    /// kira 内部管理循环逻辑，本测试验证：
    /// 1. play_bgm 返回 Ok
    /// 2. BGM 处于播放状态
    ///
    /// 真正的无缝循环效果（无间隙、无爆音）需要人耳验证（MV02）。
    #[test]
    fn ac03_bgm_seamless_loop() {
        let temp_dir = std::env::temp_dir().join("aster_test_ac03");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_ac03.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        let result = audio.play_bgm(wav_path.to_str().unwrap(), true);

        assert!(result.is_ok(), "循环 BGM 播放应返回 Ok，实际: {:?}", result);
        assert!(
            audio.is_bgm_playing(),
            "循环 BGM 播放后 is_bgm_playing 应为 true"
        );

        // 清理
        audio.stop_bgm();
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ─── AC04: BGM 停止 ────────────────────────────────────────────────

    /// AC04 — BGM 停止正常。
    ///
    /// 验证：
    /// 1. 播放 BGM 后 `is_bgm_playing()` 为 true
    /// 2. 调用 `stop_bgm()` 后 `is_bgm_playing()` 为 false
    /// 3. 对无 BGM 状态调用 `stop_bgm()` 不 panic（no-op）
    #[test]
    fn ac04_bgm_stop() {
        let temp_dir = std::env::temp_dir().join("aster_test_ac04");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_ac04.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 播放 → 停止 → 验证
        audio
            .play_bgm(wav_path.to_str().unwrap(), false)
            .expect("play_bgm 应成功");
        assert!(audio.is_bgm_playing(), "播放后应为播放中");

        audio.stop_bgm();
        assert!(!audio.is_bgm_playing(), "停止后应不在播放中");

        // 重复停止不应 panic（no-op）
        audio.stop_bgm();
        assert!(!audio.is_bgm_playing());

        // 清理
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ─── AC05: 音量实时调整 ────────────────────────────────────────────

    /// AC05 — 音量实时调整。
    ///
    /// 验证：
    /// 1. 默认音量为 0.8
    /// 2. `set_bgm_volume(0.5)` 后 `bgm_volume()` 返回 0.5
    /// 3. 音量钳制到 [0.0, 1.0] 范围
    #[test]
    fn ac05_volume_realtime_adjust() {
        let temp_dir = std::env::temp_dir().join("aster_test_ac05");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_ac05.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 默认音量
        assert!(
            (audio.bgm_volume() - 0.8).abs() < f32::EPSILON,
            "默认音量应为 0.8"
        );

        // 播放后调整音量
        audio
            .play_bgm(wav_path.to_str().unwrap(), false)
            .expect("play_bgm 应成功");
        audio.set_bgm_volume(0.5);
        assert!(
            (audio.bgm_volume() - 0.5).abs() < f32::EPSILON,
            "音量应更新为 0.5"
        );

        // 边界值测试：钳制
        audio.set_bgm_volume(1.5); // 超出上限
        assert!(
            (audio.bgm_volume() - 1.0).abs() < f32::EPSILON,
            "音量应钳制到 1.0"
        );

        audio.set_bgm_volume(-0.3); // 低于下限
        assert!(
            (audio.bgm_volume() - 0.0).abs() < f32::EPSILON,
            "音量应钳制到 0.0"
        );

        // 清理
        audio.stop_bgm();
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ─── AC06: 解码失败返回错误 ────────────────────────────────────────

    /// AC06 — 解码失败返回错误。
    ///
    /// 验证传入非音频文件（如文本文件）时：
    /// 1. 返回 `Err(AudioError::DecodeError { .. })`
    /// 2. 不 panic
    /// 3. `is_bgm_playing()` 仍为 false
    #[test]
    fn ac06_decode_error() {
        let temp_dir = std::env::temp_dir().join("aster_test_ac06");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 创建一个非音频文件（纯文本）
        let fake_audio_path = temp_dir.join("fake_audio.ogg");
        let mut file = std::fs::File::create(&fake_audio_path).unwrap();
        file.write_all(b"this is not an audio file, just plain text data")
            .unwrap();
        file.flush().unwrap();

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        let result = audio.play_bgm(fake_audio_path.to_str().unwrap(), false);

        match result {
            Err(AudioError::DecodeError { reason }) => {
                assert!(!reason.is_empty(), "DecodeError 应包含原因描述");
            }
            other => {
                panic!("应返回 DecodeError，实际返回: {:?}", other)
            }
        }

        // 验证播放状态未变
        assert!(!audio.is_bgm_playing(), "解码失败后不应有 BGM 播放");

        // 清理
        let _ = std::fs::remove_file(&fake_audio_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ─── AC07: 文件不存在返回错误 ──────────────────────────────────────

    /// AC07 — 文件不存在返回错误。
    ///
    /// 验证传入不存在的文件路径时：
    /// 1. 返回 `Err(AudioError::AssetNotFound { .. })`
    /// 2. 不 panic
    #[test]
    fn ac07_file_not_found() {
        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        let result = audio.play_bgm("nonexistent/file/path/bgm.ogg", false);

        match result {
            Err(AudioError::AssetNotFound { path }) => {
                assert!(path.contains("nonexistent"), "错误应包含原始路径");
            }
            other => panic!("应返回 AssetNotFound，实际返回: {:?}", other),
        }
    }

    // ─── AC08: BGM 替换 ──────────────────────────────────────────────────

    /// AC08 — BGM 替换：播放新 BGM 时自动停止旧 BGM。
    ///
    /// 验证：
    /// 1. 播放 BGM A → 播放 BGM B
    /// 2. BGM A 的 handle 被释放
    /// 3. BGM B 正在播放
    /// 4. BGM B 的路径被正确记录
    #[test]
    fn ac08_bgm_replace() {
        let temp_dir = std::env::temp_dir().join("aster_test_ac08");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 生成两个不同的 WAV 文件
        let wav_a = temp_dir.join("bgm_a.wav");
        let wav_b = temp_dir.join("bgm_b.wav");
        generate_test_wav(&wav_a);
        generate_test_wav(&wav_b);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 播放 BGM A
        audio
            .play_bgm(wav_a.to_str().unwrap(), false)
            .expect("play_bgm A 应成功");
        assert!(audio.is_bgm_playing(), "BGM A 应正在播放");
        assert_eq!(
            audio.current_bgm_path.as_deref(),
            Some(wav_a.to_str().unwrap()),
            "current_bgm_path 应为 BGM A 的路径"
        );

        // 播放 BGM B（应自动停止 BGM A）
        audio
            .play_bgm(wav_b.to_str().unwrap(), true)
            .expect("play_bgm B 应成功");
        assert!(audio.is_bgm_playing(), "BGM B 应正在播放");
        assert_eq!(
            audio.current_bgm_path.as_deref(),
            Some(wav_b.to_str().unwrap()),
            "current_bgm_path 应更新为 BGM B 的路径"
        );

        // 清理
        audio.stop_bgm();
        let _ = std::fs::remove_file(&wav_a);
        let _ = std::fs::remove_file(&wav_b);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ─── PH2-T02: SE 音效播放 ─────────────────────────────────────────────────

    /// AC01 — SE 播放正常。
    ///
    /// 验证调用 `play_se` 后返回 Ok，不 panic。
    #[test]
    fn ac01_se_play_normal() {
        let temp_dir = std::env::temp_dir().join("aster_test_se01");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_se01.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        let result = audio.play_se(wav_path.to_str().unwrap());

        assert!(result.is_ok(), "play_se 应返回 Ok，实际: {:?}", result);

        // 清理
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// AC02 — BGM 与 SE 同时播放互不干扰。
    ///
    /// 验证：先 play_bgm → 再 play_se → BGM 仍在播放。
    #[test]
    fn ac02_bgm_and_se_simultaneous() {
        let temp_dir = std::env::temp_dir().join("aster_test_se02");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let bgm_path = temp_dir.join("bgm_se02.wav");
        let se_path = temp_dir.join("se_se02.wav");
        generate_test_wav(&bgm_path);
        generate_test_wav(&se_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 播放 BGM
        audio
            .play_bgm(bgm_path.to_str().unwrap(), false)
            .expect("play_bgm 应成功");
        assert!(audio.is_bgm_playing(), "BGM 应正在播放");

        // 同时播放 SE（BGM 不应中断）
        let result = audio.play_se(se_path.to_str().unwrap());
        assert!(result.is_ok(), "play_se 应返回 Ok，实际: {:?}", result);
        assert!(audio.is_bgm_playing(), "SE 播放后 BGM 仍应正在播放");

        // 清理
        audio.stop_bgm();
        let _ = std::fs::remove_file(&bgm_path);
        let _ = std::fs::remove_file(&se_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// AC03 — 快速连续 SE 播放。
    ///
    /// 验证快速连续调用 play_se 5 次均返回 Ok，不 panic。
    #[test]
    fn ac03_rapid_se_playback() {
        let temp_dir = std::env::temp_dir().join("aster_test_se03");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_se03.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 快速连续播放 5 次 SE
        for i in 0..5 {
            let result = audio.play_se(wav_path.to_str().unwrap());
            assert!(
                result.is_ok(),
                "第 {} 次 play_se 应返回 Ok，实际: {:?}",
                i + 1,
                result
            );
        }

        // 清理
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// AC04 — SE 独立音量控制。
    ///
    /// 验证 set_se_volume 不影响 BGM 音量。
    #[test]
    fn ac04_se_volume_independent() {
        let temp_dir = std::env::temp_dir().join("aster_test_se04");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let bgm_path = temp_dir.join("bgm_se04.wav");
        generate_test_wav(&bgm_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 默认值
        assert!(
            (audio.bgm_volume() - 0.8).abs() < f32::EPSILON,
            "默认 BGM 音量应为 0.8"
        );
        assert!(
            (audio.se_volume() - 0.8).abs() < f32::EPSILON,
            "默认 SE 音量应为 0.8"
        );

        // 调整 SE 音量，BGM 不应变化
        audio.set_se_volume(0.3);
        assert!(
            (audio.se_volume() - 0.3).abs() < f32::EPSILON,
            "SE 音量应更新为 0.3"
        );
        assert!(
            (audio.bgm_volume() - 0.8).abs() < f32::EPSILON,
            "BGM 音量应保持 0.8 不变"
        );

        // 调整 BGM 音量，SE 不应变化
        audio.set_bgm_volume(0.5);
        assert!(
            (audio.bgm_volume() - 0.5).abs() < f32::EPSILON,
            "BGM 音量应更新为 0.5"
        );
        assert!(
            (audio.se_volume() - 0.3).abs() < f32::EPSILON,
            "SE 音量应保持 0.3 不变"
        );

        // 清理
        audio.stop_bgm();
        let _ = std::fs::remove_file(&bgm_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// AC05 — SE 静音。
    ///
    /// 验证 set_se_volume(0.0) 后 play_se 仍成功提交。
    #[test]
    fn ac05_se_mute() {
        let temp_dir = std::env::temp_dir().join("aster_test_se05");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_path = temp_dir.join("test_se05.wav");
        generate_test_wav(&wav_path);

        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // 静音后播放 SE 应仍返回 Ok（提交成功，只是音量极低）
        audio.set_se_volume(0.0);
        let result = audio.play_se(wav_path.to_str().unwrap());
        assert!(
            result.is_ok(),
            "静音状态下 play_se 仍应返回 Ok，实际: {:?}",
            result
        );

        // 清理
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// AC06 — 解码逻辑复用：BGM 和 SE 的 AssetNotFound 错误格式一致。
    ///
    /// 验证 BGM 和 SE 都通过 `load_sound_data()` 统一处理文件不存在的情况。
    #[test]
    fn ac06_se_and_bgm_share_decode_logic() {
        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };

        // BGM 文件不存在
        let bgm_result = audio.play_bgm("nonexistent/bgm.ogg", false);
        match bgm_result {
            Err(AudioError::AssetNotFound { path }) => {
                assert!(path.contains("bgm.ogg"), "BGM 错误应包含路径");
            }
            other => panic!("BGM 应返回 AssetNotFound，实际: {:?}", other),
        }

        // SE 文件不存在（应通过同一 load_sound_data 路径）
        let se_result = audio.play_se("nonexistent/se.wav");
        match se_result {
            Err(AudioError::AssetNotFound { path }) => {
                assert!(path.contains("se.wav"), "SE 错误应包含路径");
            }
            other => panic!("SE 应返回 AssetNotFound，实际: {:?}", other),
        }
    }

    // ─── 补充测试 ──────────────────────────────────────────────────────

    /// stop_bgm 在无 BGM 播放时不 panic（no-op 行为）
    #[test]
    fn stop_bgm_when_not_playing_is_noop() {
        let mut audio = match try_init_audio_system() {
            Some(a) => a,
            None => return,
        };
        // 连续调用多次 stop_bgm 不应 panic
        audio.stop_bgm();
        audio.stop_bgm();
        audio.stop_bgm();
        assert!(!audio.is_bgm_playing());
    }

    /// AudioError Display 实现不 panic
    #[test]
    fn audio_error_display_does_not_panic() {
        let errors = vec![
            AudioError::AssetNotFound {
                path: "test.ogg".into(),
            },
            AudioError::DecodeError {
                reason: "格式不支持".into(),
            },
            AudioError::PlaybackError {
                reason: "设备不可用".into(),
            },
            AudioError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "测试 IO 错误",
            )),
        ];

        for err in &errors {
            let s = format!("{}", err);
            assert!(!s.is_empty(), "错误消息不应为空");
        }
    }
}
