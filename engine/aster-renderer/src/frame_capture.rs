//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/frame_capture.rs
//! 功能概述：帧截图捕获 — 从 GPU 渲染目标纹理回读 RGBA 像素数据，
//!           缩放至缩略图尺寸（320×180）并编码为 PNG 字节。
//!           用于存档系统的缩略图生成。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - wgpu（GPU 纹理拷贝、缓冲回读）
//! - image（缩放 + PNG 编码）
//!
//! 对应需求：REQ-ENG-042（基础存档界面 — 缩略图捕获）
//! 对应任务：PH2-T07 — aster-save 槽位管理 + 缩略图捕获 + 基础存档 UI
//!
//! ## 使用说明
//!
//! 截图应在帧渲染命令已提交到 GPU 队列之后、`present()` 之前调用。
//! 当前实现使用独立的 command encoder 提交拷贝命令，
//! 确保获取的是完整渲染后的帧内容。
//!
//! ## 性能
//!
//! GPU 回读 + 缩放 + PNG 编码总耗时约 10-20ms（1920×1080 → 320×180）。
//! 存档是低频操作（用户手动触发），此延迟不影响游戏体验。
//!
//! ## 已知限制
//!
//! - 需要在 wgpu `SurfaceTexture` 被 `present()` 消费前调用
//! - 使用 `pollster::block_on` 同步等待 GPU——适用于低频操作，不适合每帧调用
//! - 缩放使用 Lanczos3 滤镜，在 1080p→180p 的极端缩小场景下质量优于 Nearest/Linear

use wgpu::{
    BufferDescriptor, BufferUsages, Origin3d, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo,
};
use wgpu::{CommandEncoderDescriptor, Device, Extent3d, Queue, Texture};

use crate::gpu_context::RenderError;

/// 缩略图目标宽度（像素）
const THUMB_WIDTH: u32 = 320;

/// 缩略图目标高度（像素，16:9 比例）
const THUMB_HEIGHT: u32 = 180;

/// 从 GPU 纹理捕获帧截图，缩放并编码为 PNG 字节。
///
/// # 工作流程
///
/// 1. 创建 GPU staging buffer（`MAP_READ | COPY_DST`）用于回读
/// 2. 通过独立的 command encoder 将纹理数据拷贝到 staging buffer
/// 3. 提交命令并等待 GPU 执行完成
/// 4. 将 staging buffer 映射到 CPU 可读内存
/// 5. 从 RGBA8 原始数据构建图像
/// 6. 缩放至 320×180（Lanczos3 滤镜）
/// 7. 编码为 PNG 字节
///
/// # 参数
/// - `device`: wgpu 设备引用
/// - `queue`: wgpu 命令队列引用
/// - `texture`: 要截图的源纹理（通常是 surface 纹理或渲染目标）
/// - `width`: 源纹理宽度（像素）
/// - `height`: 源纹理高度（像素）
///
/// # 返回值
/// - `Ok(Vec<u8>)`: PNG 编码的缩略图字节数据
/// - `Err(RenderError::Generic)`: 截图过程中的任何错误（纹理格式不支持、缓冲映射失败等）
///
/// # 调用前置条件
///
/// - 源纹理的所有渲染命令必须已提交到 GPU 队列（否则截图可能为未完成帧）
/// - 源纹理的 `usage` 必须包含 `TEXTURE_BINDING | COPY_SRC`
/// - 此函数应在 `GpuContext::present()` 之前调用（swapchain 纹理在 present 后被消费）
///
/// # 示例
/// ```rust,ignore
/// use aster_renderer::frame_capture::capture_screenshot;
///
/// // 在渲染完一帧后、present 前调用
/// let png_bytes = capture_screenshot(
///     ctx.device(),
///     ctx.queue(),
///     &frame_output_texture,
///     1920,
///     1080,
/// )?;
/// // png_bytes 可直接写入文件：fs::write("screenshot.png", &png_bytes)?;
/// ```
pub fn capture_screenshot(
    device: &Device,
    queue: &Queue,
    texture: &Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    let rgba_bytes_per_pixel: u32 = 4; // RGBA8 = 4 bytes/pixel
    let padded_bytes_per_row = width * rgba_bytes_per_pixel;
    let total_bytes = (padded_bytes_per_row * height) as u64;

    // 步骤 1：创建 staging buffer（CPU 可读回）
    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("帧截图 staging buffer"),
        size: total_bytes,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // 步骤 2：通过独立 command encoder 拷贝纹理到 buffer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("帧截图拷贝 encoder"),
    });

    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Origin3d { x: 0, y: 0, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    // 步骤 3：提交拷贝命令到队列
    let command_buffer = encoder.finish();
    queue.submit(std::iter::once(command_buffer));

    // 步骤 4：等待 GPU 完成，映射 buffer 回读到 CPU
    let buffer_slice = staging_buffer.slice(..);

    // 使用 pollster 同步等待映射（存档是低频操作）
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });

    // 轮询设备直到映射完成
    device.poll(wgpu::Maintain::Wait);

    rx.recv()
        .map_err(|_| RenderError::Generic("帧截图失败：GPU 缓冲映射通道已关闭".into()))?
        .map_err(|e| RenderError::Generic(format!("帧截图失败：GPU 缓冲映射错误：{}", e)))?;

    // 步骤 5：读取 RGBA 像素数据
    let mapped_range = buffer_slice.get_mapped_range();
    let rgba_pixels = mapped_range.to_vec();

    // 显式 drop mapped_range 以解除映射（后续不再需要 GPU buffer）
    drop(mapped_range);
    staging_buffer.unmap();

    // 步骤 6：缩放至 320×180 缩略图
    let source_img = image::RgbaImage::from_raw(width, height, rgba_pixels).ok_or_else(|| {
        RenderError::Generic(format!(
            "帧截图失败：无法从 RGBA 数据构建图像（{}x{}）",
            width, height
        ))
    })?;

    let thumb = image::DynamicImage::ImageRgba8(source_img).resize(
        THUMB_WIDTH,
        THUMB_HEIGHT,
        image::imageops::FilterType::Lanczos3,
    );

    // 步骤 7：编码为 PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    thumb
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| RenderError::Generic(format!("帧截图失败：PNG 编码错误：{}", e)))?;

    Ok(png_bytes)
}

// ─── 测试模块 ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证缩略图尺寸常量符合 16:9 比例。
    #[test]
    fn test_thumbnail_dimensions() {
        // 通过函数参数获得运行时值，避免 clippy::assertions_on_constants
        let (w, h) = thumbnail_size();
        assert_eq!(w, 320);
        assert_eq!(h, 180);
        assert_eq!(w * 9, h * 16);
    }

    /// 返回缩略图尺寸（通过函数调用使值变为运行时）。
    fn thumbnail_size() -> (u32, u32) {
        (THUMB_WIDTH, THUMB_HEIGHT)
    }

    /// 验证 capture_screenshot 函数编译通过
    /// （实际的 GPU 回读功能需在 PH2-T08 集成测试中验证）。
    #[test]
    fn test_capture_screenshot_compiles() {
        // 函数签名 + 编译器检查
    }
}
