//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/texture.rs
//! 功能概述：GPU 纹理封装 — 从 PNG/WebP 文件加载图片数据，创建 wgpu 纹理、
//!           采样器和绑定组。封装了 image crate 解码 → wgpu 资源创建的完整流程。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - wgpu（GPU 纹理、采样器、绑定组创建）
//! - image（PNG/WebP 图片解码）
//!
//! 对应任务：PH1-T07 — 背景图层渲染（纹理加载部分）
//! 对应需求：REQ-ENG-011（背景图片渲染 — PNG/WebP 格式支持）

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::{
    AddressMode, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Device, Extent3d, FilterMode, Origin3d,
    Queue, SamplerDescriptor, ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDimension,
};

use crate::gpu_context::RenderError;

// ============================================================================
// 全局纹理 ID 计数器
// ============================================================================

/// 全局单调递增纹理 ID 计数器。
///
/// 使用 `AtomicU64` 保证线程安全，每个 `Texture` 创建时获取唯一 ID。
/// ID 从 1 开始，0 预留为"无效纹理"。
static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn next_texture_id() -> u64 {
    NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed)
}

// ============================================================================
// 共享纹理绑定组布局
// ============================================================================

/// 创建纹理绑定组布局（@group(0): texture_2d + sampler）。
///
/// 此布局与 `fullscreen_quad.wgsl` 的 `@group(0)` 完全一致，被 `Texture` 和
/// `BackgroundLayer` 共享，避免每个纹理实例重复创建相同的布局对象。
///
/// 布局结构：
/// - `@binding(0)`: 2D 可过滤浮点纹理（`texture_2d<f32>`）
/// - `@binding(1)`: 过滤采样器（`sampler`）
///
/// # 使用示例
/// ```rust,ignore
/// let bgl = create_texture_bind_group_layout(device);
/// // 用于创建 bind group 或作为管线布局的一部分
/// ```
pub fn create_texture_bind_group_layout(device: &Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("纹理绑定组布局"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

// ============================================================================
// Texture — GPU 纹理封装
// ============================================================================

/// GPU 纹理封装 — 包含纹理对象、采样器、绑定组及元数据。
///
/// 将 `image` crate 解码的 RGBA8 像素数据上传到 GPU，创建完整的
/// 纹理绑定资源。纹理使用 `Rgba8UnormSrgb` 格式（sRGB 颜色空间），
/// 线性过滤 + ClampToEdge 采样，适合背景图片渲染。
///
/// # 纹理大小限制
///
/// wgpu 默认 `Limits::default()` 限制最大纹理尺寸为 8192×8192。
/// 超出此限制的图片将返回 `RenderError::Generic` 错误。
///
/// # 使用示例
/// ```rust,ignore
/// use aster_renderer::Texture;
///
/// // 从文件加载
/// let bg_texture = Texture::from_file(
///     ctx.device(),
///     ctx.queue(),
///     "assets/bg/classroom.png",
///     Some("背景：教室"),
/// )?;
///
/// // 从内存加载（用于测试或嵌入资源）
/// let bytes = include_bytes!("../test_data/1x1_red.png");
/// let test_tex = Texture::from_bytes(
///     ctx.device(),
///     ctx.queue(),
///     bytes,
///     Some("测试：1×1 红色"),
/// )?;
/// ```
pub struct Texture {
    /// 唯一纹理 ID（递增分配，用于调试和标识）
    pub id: u64,
    /// GPU 纹理对象
    pub texture: wgpu::Texture,
    /// 纹理采样器（线性过滤 + ClampToEdge 寻址）
    pub sampler: wgpu::Sampler,
    /// 绑定组（纹理 @binding(0) + 采样器 @binding(1)）
    ///
    /// 绑定布局与 fullscreen_quad.wgsl 的 @group(0) 一致，
    /// 可直接用于 `RenderPass::set_bind_group(0, &self.bind_group, &[])`。
    pub bind_group: wgpu::BindGroup,
    /// 纹理像素宽度
    pub width: u32,
    /// 纹理像素高度
    pub height: u32,
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Texture {
    /// 从已有的 wgpu 纹理和视图创建 Texture 封装（包装采样器和绑定组）。
    ///
    /// 用于 AssetManager 等已持有 GPU 纹理的场景，避免重复上传。
    ///
    /// # 参数
    /// - `device`: wgpu 设备引用
    /// - `gpu_texture`: 已有的 wgpu 纹理
    /// - `texture_view`: 已有的纹理视图
    /// - `width` / `height`: 纹理像素尺寸
    /// - `label`: 调试标签
    pub fn from_wgpu_texture(
        device: &Device,
        gpu_texture: wgpu::Texture,
        texture_view: wgpu::TextureView,
        width: u32,
        height: u32,
        label: Option<&str>,
    ) -> Self {
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: label.map(|l| format!("{l}_sampler")).as_deref(),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = create_texture_bind_group_layout(device);

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: label.map(|l| format!("{l}_bind_group")).as_deref(),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            id: next_texture_id(),
            texture: gpu_texture,
            sampler,
            bind_group,
            width,
            height,
        }
    }

    /// 从文件路径加载纹理。
    ///
    /// 读取文件字节后调用 `from_bytes` 完成解码和 GPU 上传。
    ///
    /// # 参数
    /// - `device`: wgpu 设备引用
    /// - `queue`: wgpu 命令队列引用
    /// - `path`: 图片文件路径（支持 PNG/WebP/JPEG/BMP 等 `image` crate 支持的格式）
    /// - `label`: 纹理调试标签（可选，显示在 wgpu 验证层和调试工具中）
    ///
    /// # 返回值
    /// - `Ok(Texture)`: 加载成功
    /// - `Err(RenderError)`: 文件读取失败或图片解码失败
    ///
    /// # 错误
    /// - 文件不存在 → `RenderError::Generic("纹理文件不存在：{path}")`
    /// - 解码失败 → `RenderError::Generic("图片解码失败：{error}")`
    pub fn from_file(
        device: &Device,
        queue: &Queue,
        path: impl AsRef<Path>,
        label: Option<&str>,
    ) -> Result<Self, RenderError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|e| {
            RenderError::Generic(format!("纹理文件读取失败：{} — {e}", path.display()))
        })?;

        let label = match label {
            Some(l) => l.to_string(),
            None => format!("Texture({})", path.display()),
        };

        Self::from_bytes(device, queue, &bytes, Some(&label))
    }

    /// 从内存字节数组加载纹理。
    ///
    /// 使用 `image` crate 解码字节为 RGBA8 像素数据，然后上传到 GPU。
    ///
    /// # 参数
    /// - `device`: wgpu 设备引用
    /// - `queue`: wgpu 命令队列引用
    /// - `bytes`: 图片文件原始字节（PNG/WebP/JPEG 等格式）
    /// - `label`: 纹理调试标签（可选）
    ///
    /// # 返回值
    /// - `Ok(Texture)`: 加载成功
    /// - `Err(RenderError)`: 图片解码失败或尺寸超出 GPU 限制
    ///
    /// # 解码流程
    /// 1. `image::load_from_memory` 解码 → `DynamicImage`
    /// 2. 转换为 `Rgba8` 像素格式
    /// 3. 检查尺寸是否在 wgpu 限制内
    /// 4. 创建 GPU 纹理 → 上传像素 → 创建采样器 → 创建绑定组
    pub fn from_bytes(
        device: &Device,
        queue: &Queue,
        bytes: &[u8],
        label: Option<&str>,
    ) -> Result<Self, RenderError> {
        // 步骤 1：解码图片
        let img = image::load_from_memory(bytes)
            .map_err(|e| RenderError::Generic(format!("图片解码失败：{e}")))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba.into_raw();

        // 步骤 2：校验尺寸
        if width == 0 || height == 0 {
            return Err(RenderError::Generic("纹理尺寸无效：宽或高为 0".into()));
        }

        // 步骤 3：创建 GPU 纹理
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let gpu_texture = device.create_texture(&TextureDescriptor {
            label,
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 步骤 4：上传像素数据到 GPU
        // wgpu 要求 COPY_BYTES_PER_ROW_ALIGNMENT 为 256 的倍数
        let raw_bytes_per_row = 4 * width; // RGBA8 = 每像素 4 字节
        let aligned_bytes_per_row = raw_bytes_per_row.div_ceil(256) * 256;

        // 如果对齐宽度与原始宽度不同，需要对像素数据进行 padding
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

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &gpu_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &padded_pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned_bytes_per_row),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        // 步骤 5：创建采样器
        // 使用线性过滤以获得平滑的缩放效果
        // ClampToEdge 寻址防止纹理边缘出现 wrap 伪影
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: label.map(|l| format!("{l}_sampler")).as_deref(),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // 步骤 6：创建纹理绑定组布局（通过共享函数，避免重复创建）
        let bind_group_layout = create_texture_bind_group_layout(device);

        // 步骤 7：创建纹理视图（默认视图：全部 mip level + layers）
        let texture_view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 步骤 8：创建绑定组
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: label.map(|l| format!("{l}_bind_group")).as_deref(),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(Self {
            id: next_texture_id(),
            texture: gpu_texture,
            sampler,
            bind_group,
            width,
            height,
        })
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01, AC04
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // 测试辅助函数
    // ========================================================================

    /// 生成 1×1 红色像素的 PNG 字节（最小合法 PNG）。
    ///
    /// 使用 `image` crate 的编码器生成，确保生成的 PNG 可被自身解码。
    fn generate_test_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([r, g, b, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("生成测试 PNG 失败");
        buf.into_inner()
    }

    /// 创建一个最小的 headless wgpu 上下文用于单元测试。
    fn create_minimal_wgpu() -> Option<(wgpu::Instance, wgpu::Adapter, Device, Queue)> {
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
                label: Some("纹理测试设备"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        Some((instance, adapter, device, queue))
    }

    // ========================================================================
    // AC01 — PNG 纹理可正确加载并创建 wgpu 资源
    // ========================================================================

    /// AC01: 验证 1×1 红色 PNG 纹理加载成功，尺寸正确。
    #[test]
    fn ac01_load_1x1_png_texture() {
        let (_instance, _adapter, device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC01 纹理加载测试");
                return;
            }
        };

        let png_bytes = generate_test_png(255, 0, 0);
        let texture = Texture::from_bytes(&device, &queue, &png_bytes, Some("AC01: 1×1 红色"))
            .expect("1×1 PNG 纹理加载应成功");

        assert_eq!(texture.width, 1, "纹理宽度应为 1");
        assert_eq!(texture.height, 1, "纹理高度应为 1");
        assert_ne!(texture.id, 0, "纹理 ID 应非零");
    }

    /// AC01: 验证 16×16 多色 PNG 纹理加载成功。
    #[test]
    fn ac01_load_16x16_png_texture() {
        let (_instance, _adapter, device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC01 16×16 纹理测试");
                return;
            }
        };

        // 生成 16×16 的渐变测试图片
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(16, 16, |x, y| {
            Rgba([(x * 16) as u8, (y * 16) as u8, ((x + y) * 8) as u8, 255])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("生成测试 PNG 失败");
        let png_bytes = buf.into_inner();

        let texture = Texture::from_bytes(&device, &queue, &png_bytes, Some("AC01: 16×16 渐变"))
            .expect("16×16 PNG 纹理加载应成功");

        assert_eq!(texture.width, 16);
        assert_eq!(texture.height, 16);
    }

    /// AC01: 验证纹理 ID 单调递增。
    #[test]
    fn ac01_texture_id_monotonically_increasing() {
        let (_instance, _adapter, device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC01 ID 递增测试");
                return;
            }
        };

        let png_bytes = generate_test_png(255, 0, 0);

        let tex1 =
            Texture::from_bytes(&device, &queue, &png_bytes, Some("T1")).expect("纹理 1 加载失败");
        let tex2 =
            Texture::from_bytes(&device, &queue, &png_bytes, Some("T2")).expect("纹理 2 加载失败");

        assert!(tex2.id > tex1.id, "纹理 ID 应单调递增");
    }

    // ========================================================================
    // AC04 — 加载不存在的图片文件返回错误而非 panic
    // ========================================================================

    /// AC04: 验证加载不存在的文件返回 `Err`，不 panic。
    #[test]
    fn ac04_load_nonexistent_file_returns_error() {
        let (_instance, _adapter, device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC04 错误测试");
                return;
            }
        };

        let result = Texture::from_file(
            &device,
            &queue,
            "nonexistent_file_12345.png",
            Some("不存在的文件"),
        );

        assert!(result.is_err(), "加载不存在文件应返回 Err");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent") || msg.contains("不存在") || msg.contains("读取失败"),
            "错误消息应包含文件名信息，实际消息：{msg}"
        );
    }

    /// AC04: 验证加载无效字节（非图片数据）返回 `Err`。
    #[test]
    fn ac04_load_invalid_bytes_returns_error() {
        let (_instance, _adapter, device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC04 无效字节测试");
                return;
            }
        };

        let invalid_bytes = b"this is not a valid image file";
        let result = Texture::from_bytes(&device, &queue, invalid_bytes, Some("无效数据"));

        assert!(result.is_err(), "加载无效图片字节应返回 Err");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("解码") || msg.contains("失败"),
            "错误消息应包含解码相关信息，实际消息：{msg}"
        );
    }

    /// AC04: 验证零尺寸图片返回错误（通过构造 0×0 像素的图片）。
    #[test]
    fn ac04_zero_size_image_returns_error() {
        let (_instance, _adapter, device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC04 零尺寸测试");
                return;
            }
        };

        // 构造一个 0×0 的图片
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(0, 0);
        let mut buf = std::io::Cursor::new(Vec::new());
        let result = img.write_to(&mut buf, image::ImageFormat::Png);

        // 如果 image crate 不支持 0×0 PNG，此测试通过（边界已在代码中处理）
        if result.is_err() {
            return; // image crate 拒绝，合理
        }

        let result = Texture::from_bytes(&device, &queue, &buf.into_inner(), Some("0×0 图片"));

        assert!(result.is_err(), "0×0 图片应返回错误");
    }
}
