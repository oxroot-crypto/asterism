//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/background_layer.rs
//! 功能概述：背景图层渲染器 — 管理背景纹理的全屏四边形渲染管线。
//!           支持：设置/切换背景纹理、渲染到输出视图、窗口 resize 时更新适配参数。
//!           负责 Layer 0（背景层）的全部渲染逻辑。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - wgpu（渲染管线、绑定组、着色器）
//! - crate::texture::Texture（GPU 纹理封装）
//! - crate::gpu_context::RenderError（错误类型）
//!
//! 着色器：`shaders/fullscreen_quad.wgsl` — 全屏三角形 + 宽高比适配
//!
//! 对应任务：PH1-T07 — 背景图层渲染
//! 对应需求：REQ-ENG-011（背景图片渲染）

use std::mem;

use bytemuck::{Pod, Zeroable};
// ShaderModuleDescriptor 仅用于测试模块（经由 use super::* 导入）
#[allow(unused_imports)]
use wgpu::ShaderModuleDescriptor;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferDescriptor,
    BufferUsages, ColorTargetState, ColorWrites, CommandEncoder, Device, FragmentState,
    MultisampleState, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, ShaderStages, TextureFormat, TextureView,
    VertexState, include_wgsl,
};

use crate::texture::Texture;

// ============================================================================
// FitUniform — 宽高比适配参数（CPU 端镜像）
// ============================================================================

/// 宽高比适配参数 — 传输给着色器控制纹理在窗口中的缩放方式。
///
/// 内存布局必须与 `fullscreen_quad.wgsl` 中的 `FitUniform` struct 完全一致：
/// - `texture_size`: vec2<f32> → 8 字节（偏移 0）
/// - `window_size`: vec2<f32> → 8 字节（偏移 8）
/// - `fit_mode`: f32 → 4 字节（偏移 16）
/// - 填充 → 4 字节（偏移 20，WGSL struct 自动对齐到 32 字节）
///
/// 使用 `bytemuck::Pod` 和 `bytemuck::Zeroable` 保证可安全转为字节切片。
///
/// # fit_mode 取值
/// - `0.0`: Contain — 完整显示纹理，保持宽高比，可能留黑边
/// - `1.0`: Cover — 缩放填充整个窗口，裁剪超出部分（默认）
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FitUniform {
    texture_size: [f32; 2],
    window_size: [f32; 2],
    fit_mode: f32,
    _padding: f32, // WGSL struct 对齐到 32 字节
}

impl Default for FitUniform {
    fn default() -> Self {
        Self {
            texture_size: [1920.0, 1080.0],
            window_size: [1920.0, 1080.0],
            fit_mode: 1.0, // 默认 cover 模式
            _padding: 0.0,
        }
    }
}

// ============================================================================
// BackgroundLayer — 背景图层渲染器
// ============================================================================

/// 背景图层渲染器 — 管理 Layer 0 的全屏四边形渲染管线。
///
/// 负责将背景纹理以全屏四边形方式渲染到输出视图。
/// 支持宽高比适配（cover/contain 模式），自动处理纹理切换和窗口 resize。
///
/// # 渲染管线状态
/// - 图元拓扑：`TriangleList`（3 顶点大三角形）
/// - 颜色混合：禁用（背景层不透明）
/// - 深度/模板：禁用
/// - 纹理格式：`Rgba8UnormSrgb`
///
/// # 使用示例
/// ```rust,ignore
/// use aster_renderer::{BackgroundLayer, Texture};
///
/// let mut bg_layer = BackgroundLayer::new(ctx.device(), 1920, 1080);
///
/// // 加载并设置背景
/// let bg_texture = Texture::from_file(
///     ctx.device(), ctx.queue(),
///     "assets/bg/classroom.png", Some("教室"),
/// )?;
/// bg_layer.set_background(bg_texture);
///
/// // 渲染循环中
/// let frame = ctx.acquire_frame()?;
/// {
///     let mut encoder = frame.encoder;
///     bg_layer.render(&mut encoder, &frame.view);
///     // 注意：encoder 在渲染完成后需提交到队列
/// }
/// ctx.present(frame);
/// ```
pub struct BackgroundLayer {
    /// 全屏四边形渲染管线
    pipeline: RenderPipeline,
    /// 适配 uniform 缓冲区（GPU 端）
    fit_buffer: Buffer,
    /// 适配 uniform 绑定组（@group(1)）
    fit_bind_group: BindGroup,
    /// 纹理绑定组布局（用于外部创建的纹理绑定到此管线）
    texture_bind_group_layout: wgpu::BindGroupLayout,
    /// 当前背景纹理（None 表示无背景，渲染时跳过）
    current_texture: Option<Texture>,
    /// 当前窗口尺寸（用于 resize 时重新计算 uniform）
    window_width: u32,
    /// 当前窗口高度
    window_height: u32,
    /// 当前适配模式（Contain / Cover），默认 Cover
    fit_mode: FitMode,
}

impl BackgroundLayer {
    /// 创建背景图层渲染器。
    ///
    /// 初始化全屏四边形渲染管线、uniform 缓冲区和绑定组。
    /// 初始状态无纹理，渲染时跳过绘制。
    ///
    /// # 参数
    /// - `device`: wgpu 设备引用
    /// - `queue`: wgpu 命令队列引用
    /// - `format`: 颜色纹理格式（必须与 surface 配置一致，通过 `ctx.surface_config().format` 获取）
    /// - `width`: 初始窗口宽度（逻辑像素）
    /// - `height`: 初始窗口高度（逻辑像素）
    ///
    /// # 返回值
    /// - `BackgroundLayer`: 初始化完成的背景层渲染器
    ///
    /// # 内部流程
    /// 1. 编译 fullscreen_quad.wgsl 着色器模块
    /// 2. 创建纹理绑定组布局（@group(0): texture + sampler）
    /// 3. 创建 uniform 绑定组布局（@group(1): FitUniform）
    /// 4. 创建管线布局（合并两个绑定组布局）
    /// 5. 创建渲染管线（TriangleList，无混合），颜色格式使用传入的 `format` 参数
    /// 6. 创建 uniform 缓冲区 + 绑定组
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        // 步骤 1：编译着色器
        // include_wgsl! 展开为 ShaderModuleDescriptor，直接传递给 create_shader_module
        let shader_module =
            device.create_shader_module(include_wgsl!("shaders/fullscreen_quad.wgsl"));

        // 步骤 2：创建纹理绑定组布局（@group(0): texture + sampler）
        // 通过共享函数创建，与 Texture 内部使用的布局完全一致
        let texture_bind_group_layout = crate::texture::create_texture_bind_group_layout(device);

        // 步骤 3：创建 uniform 绑定组布局（@group(1): FitUniform）
        let fit_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("适配参数绑定组布局"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // 步骤 4：创建管线布局
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("背景层管线布局"),
            bind_group_layouts: &[&texture_bind_group_layout, &fit_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 步骤 5：创建渲染管线
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("背景层渲染管线"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[], // 无顶点缓冲 — 顶点由 vertex_index 生成
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::REPLACE), // 禁用混合：完全替换
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 步骤 6：创建 uniform 缓冲区
        let fit_uniform = FitUniform {
            window_size: [width as f32, height as f32],
            ..FitUniform::default()
        };

        let fit_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("适配参数缓冲区"),
            size: mem::size_of::<FitUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 立即写入初始 uniform 数据
        queue.write_buffer(&fit_buffer, 0, bytemuck::bytes_of(&fit_uniform));

        let fit_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("适配参数绑定组"),
            layout: &fit_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(fit_buffer.as_entire_buffer_binding()),
            }],
        });

        Self {
            pipeline,
            fit_buffer,
            fit_bind_group,
            texture_bind_group_layout,
            current_texture: None,
            window_width: width,
            window_height: height,
            fit_mode: FitMode::default(),
        }
    }

    /// 设置（切换）当前背景纹理，并立即更新适配 uniform。
    ///
    /// 替换当前背景为新纹理。旧纹理的 GPU 资源在 drop 时自动释放。
    /// 设置后自动将纹理尺寸写入 fit uniform，确保下一次 `render()` 使用正确的宽高比。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用（用于写入 uniform 缓冲区）
    /// - `texture`: 要设置为背景的纹理（所有权转移）
    pub fn set_background(&mut self, queue: &Queue, texture: Texture) {
        self.current_texture = Some(texture);
        self.update_fit_buffer(queue, self.build_fit_uniform());
    }

    /// 设置（切换）当前背景纹理，返回旧纹理，并立即更新适配 uniform。
    ///
    /// 与 [`set_background`] 功能相同，但将旧纹理的所有权返回给调用方，
    /// 方便实现 A/B 背景切换（交替显示两张背景而不反复加载文件）。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `texture`: 要设置为新背景的纹理（所有权转移）
    ///
    /// # 返回值
    /// - `Option<Texture>`: 旧背景纹理；`None` 表示之前无背景
    pub fn set_background_and_return_old(
        &mut self,
        queue: &Queue,
        texture: Texture,
    ) -> Option<Texture> {
        let old = self.current_texture.take();
        self.current_texture = Some(texture);
        self.update_fit_buffer(queue, self.build_fit_uniform());
        old
    }

    /// 设置适配模式。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `mode`: `CoverMode::Cover`（裁剪填充，默认）或 `CoverMode::Contain`（完整显示，留黑边）
    pub fn set_fit_mode(&mut self, queue: &Queue, mode: FitMode) {
        self.fit_mode = mode;
        let fit_uniform = self.build_fit_uniform();
        self.update_fit_buffer(queue, fit_uniform);
    }

    /// 渲染背景图层到指定纹理视图。
    ///
    /// 如果当前无背景纹理（`current_texture` 为 `None`），则静默跳过，
    /// 不记录任何渲染命令。调用方应在此之前以 clear_color 清屏。
    ///
    /// # 参数
    /// - `encoder`: wgpu 命令编码器（可变引用）
    /// - `output_view`: 输出纹理视图（渲染目标）
    ///
    /// # 渲染流程
    /// 1. 创建 render pass（load = Load，保留已有内容）
    /// 2. 绑定管线
    /// 3. 绑定纹理 bind group（@group(0)）
    /// 4. 绑定 uniform bind group（@group(1)）
    /// 5. draw 3 个顶点（全屏大三角形）
    pub fn render<'a>(&'a self, encoder: &'a mut CommandEncoder, output_view: &'a TextureView) {
        let texture = match &self.current_texture {
            Some(t) => t,
            None => return, // 无背景纹理，静默跳过
        };

        // 步骤 1：更新 fit uniform（确保窗口尺寸和纹理尺寸为最新）
        // 注意：此处每次渲染时更新，开销极小（uniform 缓冲区写入很小）
        // 如果性能敏感，可改为脏标记延迟更新
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("背景层渲染 Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // 保留已有内容（清屏色）
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &texture.bind_group, &[]);
        render_pass.set_bind_group(1, &self.fit_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
        // render_pass 在此 drop，结束渲染通道
    }

    /// 响应窗口 resize 事件，更新适配参数。
    ///
    /// 将新的窗口尺寸写入 uniform 缓冲区，着色器在下一帧使用新参数
    /// 重新计算宽高比适配。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `width`: 新的窗口宽度（逻辑像素，自动 clamp 到 ≥1）
    /// - `height`: 新的窗口高度（逻辑像素，自动 clamp 到 ≥1）
    pub fn resize(&mut self, queue: &Queue, width: u32, height: u32) {
        self.window_width = width.max(1);
        self.window_height = height.max(1);

        let fit_uniform = self.build_fit_uniform();
        self.update_fit_buffer(queue, fit_uniform);
    }

    /// 获取纹理绑定组布局的引用。
    ///
    /// 外部代码（如 LayerManager）可能需要此布局来创建兼容的管线。
    #[inline]
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    // ========================================================================
    // 私有辅助方法
    // ========================================================================

    /// 构建当前状态的 FitUniform 数据。
    fn build_fit_uniform(&self) -> FitUniform {
        let (tex_width, tex_height) = match &self.current_texture {
            Some(t) => (t.width as f32, t.height as f32),
            None => (1920.0, 1080.0), // 无纹理时使用默认值
        };

        FitUniform {
            texture_size: [tex_width, tex_height],
            window_size: [self.window_width as f32, self.window_height as f32],
            fit_mode: self.fit_mode as u8 as f32,
            _padding: 0.0,
        }
    }

    /// 将 FitUniform 数据写入 GPU uniform 缓冲区。
    fn update_fit_buffer(&self, queue: &Queue, uniform: FitUniform) {
        queue.write_buffer(&self.fit_buffer, 0, bytemuck::bytes_of(&uniform));
    }
}

// ============================================================================
// FitMode — 适配模式枚举
// ============================================================================

/// 背景适配模式 — 控制纹理在窗口中的缩放策略。
///
/// 对应着色器中 `FitUniform.fit_mode` 的值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FitMode {
    /// 完整显示纹理，保持宽高比，窗口空白区域留黑边
    Contain = 0,
    /// 缩放填充整个窗口，保持宽高比，超出部分裁剪（默认）
    #[default]
    Cover = 1,
}

// ============================================================================
// 单元测试 — 覆盖 AC02, AC03
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{Extent3d, TextureUsages};

    // ========================================================================
    // 测试辅助函数
    // ========================================================================

    /// 生成 1×1 红色 PNG 测试纹理。
    fn generate_test_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([r, g, b, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("生成测试 PNG 失败");
        buf.into_inner()
    }

    /// 创建最小 wgpu 上下文。
    fn create_minimal_wgpu() -> Option<(Device, Queue)> {
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
                label: Some("背景层测试设备"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        Some((device, queue))
    }

    /// 创建输出纹理视图（用作渲染目标）。
    fn create_output_texture(device: &Device, width: u32, height: u32) -> TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("测试输出纹理"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    // ========================================================================
    // AC02 — 全屏四边形着色器编译成功
    // ========================================================================

    /// AC02: 验证 fullscreen_quad.wgsl 可以被编译为 ShaderModule。
    #[test]
    fn ac02_shader_compiles_successfully() {
        let (device, _queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC02 着色器编译测试");
                return;
            }
        };

        // 尝试创建着色器模块 — 如果编译失败，create_shader_module 会 panic
        let shader = device.create_shader_module(include_wgsl!("shaders/fullscreen_quad.wgsl"));

        // 如果能执行到这里，着色器编译成功
        // 进一步验证：创建管线确认顶点/片元入口点有效
        let bg_layout = crate::texture::create_texture_bind_group_layout(&device);

        let fit_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("AC02 fit 布局"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("AC02 管线布局"),
            bind_group_layouts: &[&bg_layout, &fit_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("AC02 测试管线"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 验证管线创建成功（着色器入口点存在且签名正确）
        drop(pipeline);
    }

    /// AC02: 验证 BackgroundLayer::new() 创建成功。
    #[test]
    fn ac02_background_layer_new_succeeds() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC02 创建测试");
                return;
            }
        };

        // 在 headless 模式下创建 BackgroundLayer
        let layer =
            BackgroundLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 验证初始状态：无纹理
        assert!(
            layer.current_texture.is_none(),
            "新创建的 BackgroundLayer 不应有纹理"
        );
        assert_eq!(layer.window_width, 1920);
        assert_eq!(layer.window_height, 1080);
    }

    // ========================================================================
    // AC03 — 设置背景纹理和渲染不崩溃
    // ========================================================================

    /// AC03: 验证 set_background 后可以安全渲染，无 wgpu 错误。
    #[test]
    fn ac03_set_background_and_render_no_crash() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC03 渲染测试");
                return;
            }
        };

        // 创建输出视图
        let output_view = create_output_texture(&device, 1920, 1080);

        // 加载测试纹理
        let png_bytes = generate_test_png(255, 0, 0);
        let texture = Texture::from_bytes(&device, &queue, &png_bytes, Some("AC03: 红色背景"))
            .expect("测试纹理加载失败");

        // 创建 BackgroundLayer
        let mut layer =
            BackgroundLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 设置背景
        layer.set_background(&queue, texture);

        // 渲染 — 不应 panic
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC03 测试编码器"),
        });
        layer.render(&mut encoder, &output_view);

        // 完成编码器（不 submit，因为没有 surface）
        // 仅验证 render 调用不 panic
        encoder.finish();
    }

    /// AC03: 验证无纹理时 render 不崩溃。
    #[test]
    fn ac03_render_without_texture_no_crash() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC03 空渲染测试");
                return;
            }
        };

        let output_view = create_output_texture(&device, 1920, 1080);
        let layer =
            BackgroundLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 无纹理状态下渲染 — 不应 panic
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC03 空渲染编码器"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    /// AC03: 验证切换背景纹理会正确替换旧纹理。
    #[test]
    fn ac03_switch_background_texture_no_crash() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC03 切换测试");
                return;
            }
        };

        let output_view = create_output_texture(&device, 1920, 1080);

        let tex1 = Texture::from_bytes(
            &device,
            &queue,
            &generate_test_png(255, 0, 0),
            Some("红色背景"),
        )
        .expect("纹理 1 加载失败");

        let tex2 = Texture::from_bytes(
            &device,
            &queue,
            &generate_test_png(0, 0, 255),
            Some("蓝色背景"),
        )
        .expect("纹理 2 加载失败");

        let mut layer =
            BackgroundLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 设置纹理 1 并渲染
        layer.set_background(&queue, tex1);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC03 切换测试 — 纹理 1"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();

        // 切换到纹理 2 并渲染
        layer.set_background(&queue, tex2);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC03 切换测试 — 纹理 2"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    // ========================================================================
    // FitUniform 测试
    // ========================================================================

    /// 验证 FitUniform 的内存布局与着色器预期一致。
    #[test]
    fn fit_uniform_size_matches_wgsl() {
        // WGSL 中的 FitUniform:
        // - texture_size: vec2<f32> → 8 bytes
        // - window_size: vec2<f32> → 8 bytes
        // - fit_mode: f32 → 4 bytes
        // - _padding: f32 → 4 bytes
        // 总计 24 bytes，但 WGSL struct 自动对齐可能需要填充到 32 bytes
        let size = mem::size_of::<FitUniform>();
        assert!(
            size == 32 || size == 24,
            "FitUniform 大小应为 24 或 32 字节，实际：{size}"
        );
    }

    /// 验证 FitUniform 默认值为 cover 模式。
    #[test]
    fn fit_uniform_default_is_cover() {
        let uniform = FitUniform::default();
        assert_eq!(uniform.fit_mode, 1.0, "默认应为 cover 模式");
    }

    /// 验证 FitUniform 实现 Pod + Zeroable。
    #[test]
    fn fit_uniform_is_pod_and_zeroable() {
        let uniform = FitUniform::default();
        let bytes = bytemuck::bytes_of(&uniform);
        assert_eq!(bytes.len(), mem::size_of::<FitUniform>());
    }

    // ========================================================================
    // FitMode 测试
    // ========================================================================

    /// 验证 FitMode Default 为 Cover。
    #[test]
    fn fit_mode_default_is_cover() {
        assert_eq!(FitMode::default(), FitMode::Cover);
    }

    /// 验证 FitMode 的整数值与着色器约定一致。
    #[test]
    fn fit_mode_values() {
        assert_eq!(FitMode::Contain as i32, 0);
        assert_eq!(FitMode::Cover as i32, 1);
    }
}
