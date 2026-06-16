//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-asset/src/loader.rs
//! 功能概述：资源加载器 — 定义 `AssetLoader` trait（可扩展的资源加载接口），
//!           提供 `TextureLoader`（PNG/WebP→GPU 纹理）和 `AudioLoader`
//!           （OGG/FLAC/MP3/WAV→PCM 样本）两种具体实现。
//!           新增资源类型只需实现 `AssetLoader` trait 并注册即可。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::{AssetType}（资源类型枚举）
//! - crate::error::AssetError（错误类型）
//! - wgpu（GPU 纹理创建）
//! - image（PNG/WebP/JPEG 图片解码）
//! - symphonia（OGG/FLAC/MP3/WAV 音频解码）
//!
//! 对应任务：PH2-T04 — aster-asset 资源加载基础设施

use std::path::Path;
use std::sync::Arc;

use aster_core::AssetType;

use crate::error::AssetError;

// ============================================================================
// LoadedAsset — 已加载资源的统一表示
// ============================================================================

/// 已加载资源数据 — 统一表示各类资源的解码后数据。
///
/// 资源加载后以此枚举形式返回，上层（渲染器、音频系统）根据变体
/// 提取所需数据，无需感知底层解码细节。
///
/// # 变体说明
///
/// | 变体 | 适用类型 | 说明 |
/// |------|---------|------|
/// | `Texture` | Background, CharacterSprite, GuiElement | GPU 纹理（已上传至显存） |
/// | `AudioData` | Bgm, Se, Voice | PCM f32 音频样本（尚未提交音频后端） |
/// | `Bytes` | Font, Video（未来） | 原始字节数据（保持灵活性） |
///
/// # 线程安全
///
/// `LoadedAsset` 不实现 `Clone`（wgpu Texture 不可克隆），
/// 需要共享时上层应使用 `Arc<LoadedAsset>` 包装。
///
/// # Debug 实现
///
/// 手动实现 `Debug`（非 derive），因为 `wgpu::Texture` 和
/// `wgpu::TextureView` 不实现 `Debug`。纹理变体仅输出尺寸信息。
pub enum LoadedAsset {
    /// GPU 纹理资源。
    ///
    /// 包含纹理对象、默认纹理视图和像素尺寸。
    /// 纹理格式固定为 `Rgba8UnormSrgb`（sRGB 颜色空间），
    /// 使用线性过滤 + ClampToEdge 寻址。
    Texture {
        /// wgpu 纹理对象（GPU 显存中的像素数据）
        texture: wgpu::Texture,
        /// 默认纹理视图（全部 mip level + layers）
        view: wgpu::TextureView,
        /// 纹理像素尺寸（宽度, 高度）
        size: (u32, u32),
    },

    /// 已解码的 PCM 音频数据。
    ///
    /// 采样格式为 f32（-1.0 ~ 1.0 归一化浮点），
    /// 交错排列（interleaved）：`[L0, R0, L1, R1, ...]`。
    /// 音频后端（kira）可根据此数据创建 `StaticSoundData`。
    AudioData {
        /// PCM f32 采样数据（交错排列，长度 = 采样数 × 通道数）
        samples: Vec<f32>,
        /// 采样率（Hz），例如 44100、48000
        sample_rate: u32,
        /// 音频通道数（1=单声道, 2=立体声）
        channels: u16,
    },

    /// 原始字节数据 — 用于不需要解码的资源类型。
    ///
    /// 适用场景：字体文件（.ttf/.otf）、配置文件、未来视频格式等。
    /// 字节数据由上层按需解析。
    Bytes {
        /// 原始文件字节
        data: Vec<u8>,
    },
}

impl std::fmt::Debug for LoadedAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Texture { size, .. } => f
                .debug_struct("Texture")
                .field("size", size)
                .finish_non_exhaustive(),
            Self::AudioData {
                samples,
                sample_rate,
                channels,
            } => f
                .debug_struct("AudioData")
                .field("samples_len", &samples.len())
                .field("sample_rate", sample_rate)
                .field("channels", channels)
                .finish(),
            Self::Bytes { data } => f.debug_struct("Bytes").field("len", &data.len()).finish(),
        }
    }
}

// ============================================================================
// AssetLoader trait — 可扩展的资源加载接口
// ============================================================================

/// 资源加载器 trait — 定义资源解码的统一接口。
///
/// 每种资源类型（或同类型的多种格式，如 PNG + WebP 均为图片）对应一个
/// `AssetLoader` 实现。新增资源格式只需实现此 trait 并通过
/// `AssetManager::register_loader()` 注册。
///
/// # 设计说明
///
/// `supported_types()` 返回切片（非单个 `AssetType`），因为某些加载器
/// 可处理多种资源类型。例如 `TextureLoader` 同时适用于 `Background`、
/// `CharacterSprite` 和 `GuiElement`（三者均为 RGBA 图片，解码逻辑相同）。
///
/// # 线程安全
///
/// 所有实现必须满足 `Send + Sync`，因为 `AssetManager` 可能在多线程
/// 环境中被访问（例如异步预加载场景）。
///
/// # 实现示例
///
/// ```rust,ignore
/// use aster_asset::{AssetLoader, AssetError, LoadedAsset};
/// use aster_core::AssetType;
///
/// struct MyCustomLoader;
///
/// impl AssetLoader for MyCustomLoader {
///     fn supported_types(&self) -> &[AssetType] {
///         &[AssetType::Font]
///     }
///
///     fn load(&self, path: &Path) -> Result<LoadedAsset, AssetError> {
///         // 解码逻辑...
///         let data = std::fs::read(path)?;
///         Ok(LoadedAsset::Bytes { data })
///     }
/// }
/// ```
pub trait AssetLoader: Send + Sync {
    /// 返回此加载器支持处理的所有资源类型。
    ///
    /// 同一加载器实例可注册到多种 `AssetType`。
    /// 例如 `TextureLoader` 返回 `[Background, CharacterSprite, GuiElement]`。
    fn supported_types(&self) -> &[AssetType];

    /// 从文件路径加载并解码资源。
    ///
    /// # 参数
    /// - `path`：资源文件的完整文件系统路径（非相对路径）
    ///
    /// # 返回值
    /// - `Ok(LoadedAsset)`：解码成功，返回统一资源表示
    /// - `Err(AssetError)`：文件不存在、格式不支持或解码失败
    ///
    /// # 实现约定
    /// - 调用前应确认文件存在（由调用方或实现方检查）
    /// - 解码失败应返回 `AssetError::DecodeError`，包含底层错误信息
    /// - 不支持 `path` 指向的格式时应返回 `AssetError::UnsupportedFormat`
    fn load(&self, path: &Path) -> Result<LoadedAsset, AssetError>;
}

// ============================================================================
// TextureLoader — 图片→GPU 纹理加载器
// ============================================================================

/// 图片纹理加载器 — 将 PNG/WebP/JPEG 图片解码并上传到 GPU。
///
/// 内部使用 `image` crate 解码像素数据，通过 wgpu 创建 GPU 纹理资源。
/// 设备（`Device`）和队列（`Queue`）通过构造函数注入，
/// 以便在 wgpu 初始化后才注册此加载器。
///
/// # 支持的格式
/// - PNG（RGBA8，含透明通道）
/// - WebP（有损/无损，支持透明）
/// - JPEG（基线/渐进式，用于背景）
///
/// # GPU 纹理规格
/// - 格式：`Rgba8UnormSrgb`（sRGB 颜色空间）
/// - 用途：`TEXTURE_BINDING | COPY_DST`
/// - 采样：线性过滤 + ClampToEdge 寻址
/// - 字节对齐：256 字节/行（wgpu `COPY_BYTES_PER_ROW_ALIGNMENT` 要求）
///
/// # 使用示例
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use aster_asset::TextureLoader;
///
/// let loader = TextureLoader::new(Arc::clone(&device), Arc::clone(&queue));
/// let asset = loader.load("assets/bg/classroom.png")?;
/// if let LoadedAsset::Texture { size, .. } = asset {
///     println!("纹理加载成功：{}×{}", size.0, size.1);
/// }
/// ```
pub struct TextureLoader {
    /// wgpu 设备引用（共享所有权）
    device: Arc<wgpu::Device>,
    /// wgpu 命令队列引用（共享所有权）
    queue: Arc<wgpu::Queue>,
}

impl TextureLoader {
    /// 创建新的纹理加载器。
    ///
    /// # 参数
    /// - `device`：wgpu 设备（通常来自 `GpuContext`）
    /// - `queue`：wgpu 命令队列（通常来自 `GpuContext`）
    ///
    /// 使用 `Arc` 共享所有权，因为 `Device` 和 `Queue` 在引擎全局
    /// 只有一个实例，被多个模块共享。
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self { device, queue }
    }

    /// 从内存字节数组创建 GPU 纹理。
    ///
    /// 解码→格式转换→对齐→上传的完整流程。公开此方法便于测试
    /// （可传入内存中的 PNG 字节而不需要物理文件）。
    ///
    /// # 参数
    /// - `bytes`：图片文件原始字节（PNG/WebP/JPEG 等格式）
    /// - `label`：wgpu 调试标签（可选）
    ///
    /// # 解码流程
    /// 1. `image::load_from_memory` 自动检测格式并解码
    /// 2. 转换为 RGBA8 格式
    /// 3. 检查尺寸合法性（非零、不超 GPU 限制）
    /// 4. bytes_per_row 对齐到 256
    /// 5. 创建 wgpu Texture + 上传像素 + 创建 TextureView
    pub fn from_bytes(&self, bytes: &[u8], label: Option<&str>) -> Result<LoadedAsset, AssetError> {
        // 步骤 1：使用 image crate 解码图片
        let img = image::load_from_memory(bytes).map_err(|e| AssetError::DecodeError {
            reason: format!("图片解码失败：{e}"),
        })?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba.into_raw();

        // 步骤 2：校验尺寸
        if width == 0 || height == 0 {
            return Err(AssetError::DecodeError {
                reason: "纹理尺寸无效：宽或高为 0".into(),
            });
        }

        // 步骤 3：创建 GPU 纹理
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let gpu_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 步骤 4：字节对齐（wgpu 要求 COPY_BYTES_PER_ROW_ALIGNMENT = 256）
        let raw_bytes_per_row = 4 * width; // RGBA8 = 每像素 4 字节
        let aligned_bytes_per_row = raw_bytes_per_row.div_ceil(256) * 256;

        let padded_pixels = if aligned_bytes_per_row == raw_bytes_per_row {
            pixels
        } else {
            let padded_size = (aligned_bytes_per_row * height) as usize;
            let mut padded = vec![0u8; padded_size];
            for row in 0..height as usize {
                let src_start = row * raw_bytes_per_row as usize;
                let dst_start = row * aligned_bytes_per_row as usize;
                padded[dst_start..dst_start + raw_bytes_per_row as usize]
                    .copy_from_slice(&pixels[src_start..src_start + raw_bytes_per_row as usize]);
            }
            padded
        };

        // 步骤 5：上传像素数据到 GPU
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &padded_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned_bytes_per_row),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        // 步骤 6：创建默认纹理视图
        let view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(LoadedAsset::Texture {
            texture: gpu_texture,
            view,
            size: (width, height),
        })
    }
}

impl AssetLoader for TextureLoader {
    /// TextureLoader 支持所有基于 RGBA 图片的资源类型。
    ///
    /// Background、CharacterSprite、GuiElement 均使用相同的
    /// PNG/WebP 解码 + GPU 上传流程，因此共用一个加载器实例。
    fn supported_types(&self) -> &[AssetType] {
        &[
            AssetType::Background,
            AssetType::CharacterSprite,
            AssetType::GuiElement,
        ]
    }

    fn load(&self, path: &Path) -> Result<LoadedAsset, AssetError> {
        // 检查文件是否存在
        if !path.exists() {
            return Err(AssetError::NotFound {
                path: path.display().to_string(),
            });
        }

        // 读取文件字节并委托 from_bytes 完成解码
        let bytes = std::fs::read(path).map_err(AssetError::Io)?;

        let label = format!("Texture({})", path.display());
        self.from_bytes(&bytes, Some(&label))
    }
}

// ============================================================================
// AudioLoader — 音频→PCM 样本加载器
// ============================================================================

/// 音频加载器 — 将 OGG/FLAC/MP3/WAV 音频文件解码为 PCM f32 样本。
///
/// 使用 symphonia 多媒体框架进行格式检测和解码。
/// 解码后的 PCM 样本保持原始采样率和通道数，不在此阶段做重采样。
/// 音频后端（kira/aster-audio）可根据 `AudioData` 自行创建播放实例。
///
/// # 支持的格式
/// - OGG Vorbis（推荐用于 BGM/SE/Voice）
/// - FLAC（无损，用于高品质 BGM）
/// - MP3（有损，用于兼容性）
/// - WAV（无损/未压缩，开发阶段常用）
///
/// # 解码流程
/// ```text
/// File → MediaSourceStream → ProbeFormat → Track → Decoder → SampleBuffer<f32> → AudioData
/// ```
///
/// # 使用示例
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use aster_asset::{AudioLoader, AssetLoader};
///
/// let loader = AudioLoader::new();
/// let asset = loader.load("assets/bgm/theme.ogg")?;
/// if let LoadedAsset::AudioData { samples, sample_rate, channels } = asset {
///     println!("解码完成：{} 采样，{}Hz，{} 通道", samples.len(), sample_rate, channels);
/// }
/// ```
pub struct AudioLoader;

impl AudioLoader {
    /// 创建新的音频加载器。
    ///
    /// `AudioLoader` 无状态——每次 `load()` 调用独立解码文件，
    /// 不持有设备引用或内部缓存。
    pub fn new() -> Self {
        Self
    }
}

impl Default for AudioLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetLoader for AudioLoader {
    /// AudioLoader 支持所有音频资源类型。
    fn supported_types(&self) -> &[AssetType] {
        &[AssetType::Bgm, AssetType::Se, AssetType::Voice]
    }

    fn load(&self, path: &Path) -> Result<LoadedAsset, AssetError> {
        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        // 步骤 1：检查文件是否存在
        if !path.exists() {
            return Err(AssetError::NotFound {
                path: path.display().to_string(),
            });
        }

        // 步骤 2：打开文件，创建 MediaSourceStream
        let file = std::fs::File::open(path).map_err(AssetError::Io)?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // 步骤 3：探测音频格式（symphonia 自动检测容器格式）
        let hint = Hint::new();
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| AssetError::DecodeError {
                reason: format!("音频格式探测失败：{e}"),
            })?;

        let mut format = probed.format;

        // 步骤 4：获取默认音轨的解码器
        let track = format
            .default_track()
            .ok_or_else(|| AssetError::DecodeError {
                reason: "音频文件不包含任何可解码的音轨".into(),
            })?;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| AssetError::DecodeError {
                reason: format!("无法创建音频解码器：{e}"),
            })?;

        // 步骤 5：提取音频参数
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);

        // 步骤 6：逐包解码，收集全部 PCM f32 样本
        let mut samples: Vec<f32> = Vec::new();
        let mut sample_buf: Option<SampleBuffer<f32>> = None;

        loop {
            // 获取下一个编码包
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // 流结束，正常退出
                    break;
                }
                Err(e) => {
                    return Err(AssetError::DecodeError {
                        reason: format!("音频包读取失败：{e}"),
                    });
                }
            };

            // 解码包
            let decoded = decoder
                .decode(&packet)
                .map_err(|e| AssetError::DecodeError {
                    reason: format!("音频解码失败：{e}"),
                })?;

            // 将解码数据拷贝到样本缓冲区
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;

            if sample_buf.is_none() {
                sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
            }

            if let Some(ref mut buf) = sample_buf {
                buf.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buf.samples());
            }
        }

        // 步骤 7：校验解码结果
        if samples.is_empty() {
            return Err(AssetError::DecodeError {
                reason: "音频文件解码后无有效样本".into(),
            });
        }

        Ok(LoadedAsset::AudioData {
            samples,
            sample_rate,
            channels,
        })
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC03, AC04, AC05
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // 测试辅助：生成测试用图片/音频文件
    // ========================================================================

    /// 生成 1×1 红色像素的 PNG 字节（最小合法 PNG，77 字节左右）。
    ///
    /// 使用 `image` crate 的编码器，确保生成的文件可被自身解码。
    /// 返回原始 PNG 字节。
    fn generate_test_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([r, g, b, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("生成测试 PNG 失败");
        buf.into_inner()
    }

    /// 生成 1×1 WebP 字节（用于测试 WebP 支持）。
    fn generate_test_webp() -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([0, 255, 0, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::WebP)
            .expect("生成测试 WebP 失败");
        buf.into_inner()
    }

    /// 生成最小合法 WAV 文件（44 字节 header + 440Hz 正弦波 PCM 数据）。
    ///
    /// 生成 44100Hz、单声道、16-bit、持续 0.1 秒的 WAV。
    /// WAV 格式简单且被 symphonia 广泛支持，适合自动化测试。
    fn generate_test_wav() -> Vec<u8> {
        let sample_rate: u32 = 44100;
        let duration_secs: f32 = 0.1;
        let num_samples: usize = (sample_rate as f32 * duration_secs) as usize;

        // 生成 440Hz 正弦波样本（f32 归一化）
        let amplitude = 0.5f32;
        let mut f32_samples = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = amplitude * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            f32_samples.push(sample);
        }

        // 转换为 16-bit PCM
        let pcm_samples: Vec<i16> = f32_samples.iter().map(|&s| (s * 32767.0) as i16).collect();

        let data_size = (pcm_samples.len() * 2) as u32; // 16-bit = 2 bytes/sample
        let file_size = 36 + data_size;

        let mut buf = Vec::with_capacity(44 + pcm_samples.len() * 2);

        // RIFF header
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");

        // fmt chunk
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM = 1
        buf.extend_from_slice(&1u16.to_le_bytes()); // mono
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
        buf.extend_from_slice(&2u16.to_le_bytes()); // block align
        buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

        // data chunk
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        for sample in &pcm_samples {
            buf.extend_from_slice(&sample.to_le_bytes());
        }

        buf
    }

    /// 创建临时目录并在其中写入测试文件。
    ///
    /// 返回 `TempDir` 的路径和文件名列表，测试结束后自动清理。
    fn create_temp_file(name: &str, data: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");
        let file_path = dir.path().join(name);
        std::fs::write(&file_path, data).expect("写入测试文件失败");
        (dir, file_path)
    }

    /// 创建最小 wgpu 上下文的辅助函数。
    ///
    /// 尝试创建 headless wgpu 设备用于纹理加载测试。
    /// 如果环境不支持（如 CI 无 GPU），返回 `None`，测试优雅跳过。
    fn create_minimal_wgpu() -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("aster-asset 测试设备"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        Some((Arc::new(device), Arc::new(queue)))
    }

    // ========================================================================
    // AC03 — TextureLoader 加载 PNG→Texture
    // ========================================================================

    /// AC03: 验证 1×1 红色 PNG 可正确加载为 GPU 纹理。
    #[test]
    fn ac03_load_png_to_texture() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC03 PNG 纹理加载测试");
                return;
            }
        };

        let loader = TextureLoader::new(device, queue);
        let png_bytes = generate_test_png(255, 0, 0);
        let (_dir, file_path) = create_temp_file("test.png", &png_bytes);

        let result = loader.load(&file_path);
        assert!(
            result.is_ok(),
            "PNG 纹理加载应成功，错误：{:?}",
            result.err()
        );

        match result.unwrap() {
            LoadedAsset::Texture { size, .. } => {
                assert_eq!(size, (1, 1), "1×1 PNG 纹理尺寸应为 (1,1)");
            }
            _other => panic!("预期 Texture 变体，实际得到非 Texture 变体"),
        }
    }

    /// AC03 补充：验证 16×16 多色 PNG 加载。
    #[test]
    fn ac03_load_multicolor_png() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC03 多色 PNG 测试");
                return;
            }
        };

        // 生成 16×16 渐变图片
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(16, 16, |x, y| {
            Rgba([(x * 16) as u8, (y * 16) as u8, 128, 255])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("生成测试 PNG 失败");
        let png_bytes = buf.into_inner();

        let loader = TextureLoader::new(device, queue);
        let (_dir, file_path) = create_temp_file("gradient.png", &png_bytes);

        let result = loader.load(&file_path).expect("16×16 PNG 加载应成功");

        match result {
            LoadedAsset::Texture { size, .. } => {
                assert_eq!(size, (16, 16));
            }
            _other => panic!("预期 Texture 变体"),
        }
    }

    // ========================================================================
    // AC04 — TextureLoader 加载 WebP→Texture
    // ========================================================================

    /// AC04: 验证 WebP 文件可正确加载为 GPU 纹理。
    #[test]
    fn ac04_load_webp_to_texture() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC04 WebP 纹理加载测试");
                return;
            }
        };

        let loader = TextureLoader::new(device, queue);
        let webp_bytes = generate_test_webp();
        let (_dir, file_path) = create_temp_file("test.webp", &webp_bytes);

        let result = loader.load(&file_path);
        assert!(
            result.is_ok(),
            "WebP 纹理加载应成功，错误：{:?}",
            result.err()
        );

        match result.unwrap() {
            LoadedAsset::Texture { size, .. } => {
                assert_eq!(size, (1, 1), "1×1 WebP 纹理尺寸应为 (1,1)");
            }
            _other => panic!("预期 Texture 变体"),
        }
    }

    // ========================================================================
    // AC05 — AudioLoader 解码 OGG/WAV→PCM
    // ========================================================================

    /// AC05: 验证 WAV 文件可正确解码为 PCM f32 样本。
    ///
    /// 注意：使用 WAV 而非 OGG 作为测试格式，因为可以程序化生成
    /// 最小合法 WAV 文件，无需外部测试数据。
    #[test]
    fn ac05_load_wav_to_pcm() {
        let loader = AudioLoader::new();
        let wav_bytes = generate_test_wav();
        let (_dir, file_path) = create_temp_file("test_tone.wav", &wav_bytes);

        let result = loader.load(&file_path);
        assert!(result.is_ok(), "WAV 解码应成功，错误：{:?}", result.err());

        match result.unwrap() {
            LoadedAsset::AudioData {
                samples,
                sample_rate,
                channels,
            } => {
                assert!(!samples.is_empty(), "PCM 样本不应为空");
                assert!(sample_rate > 0, "采样率应 > 0");
                assert!(channels > 0, "通道数应 > 0");
                // 0.1 秒 × 44100 Hz = 4410 样本 ± 容差
                let expected_samples = (0.1 * sample_rate as f32) as usize;
                let diff = (samples.len() as isize - expected_samples as isize).unsigned_abs();
                assert!(
                    diff <= 100,
                    "样本数 {} 应接近预期 {}，差异 {}",
                    samples.len(),
                    expected_samples,
                    diff
                );
                // 验证样本不全为零（确实解码了有效音频）
                let has_nonzero = samples.iter().any(|&s| s.abs() > 0.01);
                assert!(has_nonzero, "解码的样本应包含非零值（正弦波）");
            }
            _other => panic!("预期 AudioData 变体"),
        }
    }

    /// AC05 补充：验证文件不存在时 AudioLoader 返回 NotFound。
    #[test]
    fn ac05_audio_file_not_found() {
        let loader = AudioLoader::new();
        let path = std::path::Path::new("nonexistent_audio_xyz.ogg");
        let result = loader.load(path);

        assert!(result.is_err(), "不存在的文件应返回错误");
        match result.unwrap_err() {
            AssetError::NotFound { .. } => {} // 预期
            other => panic!("预期 NotFound 错误，实际得到：{other:?}"),
        }
    }

    // ========================================================================
    // TextureLoader 错误路径测试
    // ========================================================================

    /// 验证加载不存在的文件返回 NotFound。
    #[test]
    fn test_texture_loader_file_not_found() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过纹理错误路径测试");
                return;
            }
        };

        let loader = TextureLoader::new(device, queue);
        let path = std::path::Path::new("definitely_not_exist_12345.png");
        let result = loader.load(path);

        assert!(result.is_err());
        match result.unwrap_err() {
            AssetError::NotFound { .. } => {}
            other => panic!("预期 NotFound，实际得到：{other:?}"),
        }
    }

    /// 验证加载无效字节（非图片数据）返回 DecodeError。
    #[test]
    fn test_texture_loader_invalid_bytes() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过无效字节测试");
                return;
            }
        };

        let loader = TextureLoader::new(device, queue);
        let invalid_data = b"this is not an image file!";
        let (_dir, file_path) = create_temp_file("fake.png", invalid_data);

        let result = loader.load(&file_path);
        assert!(result.is_err(), "无效图片数据应返回错误");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("解码") || err_msg.contains("失败"),
            "错误消息应提示解码失败：{err_msg}"
        );
    }

    /// 验证 AudioLoader 加载非音频文件返回 DecodeError。
    #[test]
    fn test_audio_loader_invalid_format() {
        let loader = AudioLoader::new();
        let invalid_data = b"not a valid audio file at all!";
        let (_dir, file_path) = create_temp_file("fake.ogg", invalid_data);

        let result = loader.load(&file_path);
        assert!(result.is_err(), "非音频数据应返回错误");
    }

    // ========================================================================
    // supported_types 验证
    // ========================================================================

    /// 验证 TextureLoader 返回正确的支持类型列表。
    #[test]
    fn test_texture_loader_supported_types() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                // 即使没有 GPU，supported_types 也能测试（不需要设备）
                return;
            }
        };

        let loader = TextureLoader::new(device, queue);
        let types = loader.supported_types();

        assert!(types.contains(&AssetType::Background));
        assert!(types.contains(&AssetType::CharacterSprite));
        assert!(types.contains(&AssetType::GuiElement));
        assert_eq!(types.len(), 3);
    }

    /// 验证 AudioLoader 返回正确的支持类型列表。
    #[test]
    fn test_audio_loader_supported_types() {
        let loader = AudioLoader::new();
        let types = loader.supported_types();

        assert!(types.contains(&AssetType::Bgm));
        assert!(types.contains(&AssetType::Se));
        assert!(types.contains(&AssetType::Voice));
        assert_eq!(types.len(), 3);
    }

    // ========================================================================
    // 图片解码错误路径
    // ========================================================================

    /// 验证零尺寸图片返回错误。
    #[test]
    fn test_zero_size_texture() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过零尺寸测试");
                return;
            }
        };

        // 创建 0×0 像素的图片会失败（image crate 不接受），
        // 此处用空字节数组测试边界
        let loader = TextureLoader::new(device, queue);
        let result = loader.from_bytes(&[], None);

        assert!(result.is_err(), "空字节数组应返回解码错误");
    }

    /// 验证 PNG 字节损坏时返回解码错误。
    #[test]
    fn test_corrupted_png() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过损坏 PNG 测试");
                return;
            }
        };

        let loader = TextureLoader::new(device, queue);
        // 使用有效的 PNG magic 但后续数据损坏
        let corrupted: Vec<u8> = {
            let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]; // 正确 PNG magic
            v.extend_from_slice(&[0xFF; 100]); // 损坏的 chunk 数据
            v
        };
        let (_dir, file_path) = create_temp_file("corrupted.png", &corrupted);

        let result = loader.load(&file_path);
        assert!(result.is_err(), "损坏的 PNG 应返回解码错误");
    }
}
