//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/sprite_layer.rs
//! 功能概述：立绘精灵图层渲染器 — 管理 Layer 1/2 的角色立绘渲染。
//!           支持多个立绘同时显示，每个立绘可独立设置屏幕位置（Left/Center/Right/Custom）、
//!           缩放比例、透明度（0.0~1.0）和 z-index（层内排序）。
//!           使用 alpha 混合叠加在背景层之上。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - wgpu（渲染管线、顶点缓冲、uniform 缓冲、alpha 混合）
//! - crate::texture::Texture（GPU 纹理封装）
//! - crate::layer_manager::Layer（渲染层 trait）
//!
//! 着色器：`shaders/sprite.wgsl` — 四边形顶点变换 + alpha 混合片元着色
//!
//! 对应任务：PH1-T08 — 角色立绘渲染
//! 对应需求：REQ-ENG-012（角色立绘渲染）

use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendComponent, BlendFactor,
    BlendOperation, BlendState, Buffer, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoder, Device, FragmentState, IndexFormat, MultisampleState,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, ShaderStages, TextureFormat, TextureView, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode, include_wgsl,
};

use crate::layer_manager::Layer;
use crate::texture::Texture;

// ============================================================================
// 全局 Sprite ID 计数器
// ============================================================================

/// 全局单调递增立绘 ID 计数器。
///
/// 每个立绘创建时获取唯一 ID，用于 `remove_sprite` / `update_*` 等 API 的标识。
static NEXT_SPRITE_ID: AtomicU64 = AtomicU64::new(1);

fn next_sprite_id() -> u64 {
    NEXT_SPRITE_ID.fetch_add(1, Ordering::Relaxed)
}

// ============================================================================
// SpritePosition — 立绘屏幕位置
// ============================================================================

/// 立绘屏幕位置枚举 — 定义立绘在画面中的锚点位置。
///
/// 坐标系统使用归一化坐标（0.0~1.0），原点为屏幕左上角：
/// - x: 0.0 = 左边缘，1.0 = 右边缘
/// - y: 0.0 = 顶部，1.0 = 底部
///
/// 锚点位于立绘的底部中心（角色站在屏幕底部）。
///
/// # 预设位置
/// - `Left`    → 归一化坐标 `(0.25, 0.5)`，角色在屏幕左 1/4 处
/// - `Center`  → 归一化坐标 `(0.5, 0.5)`，角色在屏幕正中央
/// - `Right`   → 归一化坐标 `(0.75, 0.5)`，角色在屏幕右 1/4 处
/// - `Custom`  → 直接使用自定义归一化坐标
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpritePosition {
    /// 左侧位置 — 归一化坐标 (0.25, 0.5)
    Left,
    /// 中央位置 — 归一化坐标 (0.5, 0.5)
    #[default]
    Center,
    /// 右侧位置 — 归一化坐标 (0.75, 0.5)
    Right,
    /// 自定义位置 — (x, y) 归一化坐标，x ∈ [0.0, 1.0], y ∈ [0.0, 1.0]
    Custom(f32, f32),
}

impl SpritePosition {
    /// 将位置枚举转换为归一化坐标 (x, y)。
    ///
    /// # 返回值
    /// - `(x, y)`: 归一化坐标，范围 [0.0, 1.0]
    ///
    /// # 示例
    /// ```rust,ignore
    /// assert_eq!(SpritePosition::Left.to_coords(), (0.25, 0.5));
    /// assert_eq!(SpritePosition::Center.to_coords(), (0.5, 0.5));
    /// assert_eq!(SpritePosition::Right.to_coords(), (0.75, 0.5));
    /// assert_eq!(SpritePosition::Custom(0.1, 0.8).to_coords(), (0.1, 0.8));
    /// ```
    #[must_use]
    pub fn to_coords(self) -> (f32, f32) {
        match self {
            Self::Left => (0.25, 0.5),
            Self::Center => (0.5, 0.5),
            Self::Right => (0.75, 0.5),
            Self::Custom(x, y) => (x, y),
        }
    }
}

// ============================================================================
// SpriteDescriptor — 立绘创建描述符
// ============================================================================

/// 立绘创建描述符 — 添加立绘时传入的配置参数。
///
/// 使用 builder 模式构造，所有字段有合理默认值。
///
/// # 使用示例
/// ```rust,ignore
/// let desc = SpriteDescriptor::new(SpritePosition::Left)
///     .with_scale(1.0, 1.0)
///     .with_alpha(0.8)
///     .with_z_index(10);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteDescriptor {
    /// 屏幕位置
    pub position: SpritePosition,
    /// 缩放因子，1.0 = 原始尺寸（默认：1.0）
    pub scale: (f32, f32),
    /// 透明度，0.0 = 全透明，1.0 = 不透明（默认：1.0）
    pub alpha: f32,
    /// 层内排序，数值越大越靠前（默认：0）
    pub z_index: i32,
}

impl SpriteDescriptor {
    /// 创建新的立绘描述符，指定位置，其他字段使用默认值。
    ///
    /// # 参数
    /// - `position`: 立绘屏幕位置
    #[must_use]
    pub fn new(position: SpritePosition) -> Self {
        Self {
            position,
            scale: (1.0, 1.0),
            alpha: 1.0,
            z_index: 0,
        }
    }

    /// 设置缩放因子。
    #[must_use]
    pub fn with_scale(mut self, x: f32, y: f32) -> Self {
        self.scale = (x, y);
        self
    }

    /// 设置透明度。
    #[must_use]
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha.clamp(0.0, 1.0);
        self
    }

    /// 设置 z-index。
    #[must_use]
    pub fn with_z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }
}

impl Default for SpriteDescriptor {
    fn default() -> Self {
        Self::new(SpritePosition::default())
    }
}

// ============================================================================
// Sprite — 立绘数据（不含 GPU 资源）
// ============================================================================

/// 立绘数据 — 描述单个立绘的属性（不含 GPU 资源）。
///
/// GPU 资源（纹理、uniform 缓冲、绑定组）由 `SpriteLayer` 内部管理。
/// 此结构体用于外部查询立绘状态。
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    /// 唯一立绘 ID
    pub id: u64,
    /// 纹理 ID（对应 Texture.id）
    pub texture_id: u64,
    /// 屏幕位置
    pub position: SpritePosition,
    /// 缩放因子
    pub scale: (f32, f32),
    /// 透明度
    pub alpha: f32,
    /// 层内排序
    pub z_index: i32,
}

// ============================================================================
// SpriteUniform — CPU 端 uniform 数据
// ============================================================================

/// 立绘 Uniform — 传输给着色器的逐立绘变换参数。
///
/// 内存布局必须与 `sprite.wgsl` 中的 `SpriteUniform` struct 完全一致：
/// - `sprite_pos`: vec2<f32> → 8 字节（偏移 0）
/// - `sprite_half_size`: vec2<f32> → 8 字节（偏移 8）
/// - `alpha`: f32 → 4 字节（偏移 16）
/// - `_padding`: f32 → 4 字节（偏移 20，总计 24 字节）
///
/// 使用 `bytemuck::Pod` 和 `bytemuck::Zeroable` 保证可安全转为字节切片。
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SpriteUniform {
    /// 立绘中心在 NDC 空间的位置（x ∈ [-1, 1], y ∈ [-1, 1]）
    sprite_pos: [f32; 2],
    /// 立绘四边形在 NDC 空间的半尺寸（half-width, half-height）
    sprite_half_size: [f32; 2],
    /// 透明度（0.0 = 全透明，1.0 = 不透明）
    alpha: f32,
    /// WGSL struct 对齐填充
    _padding: f32,
}

/// SpriteUniform 计算参数 — 聚合计算 uniform 所需的所有输入。
struct UniformParams {
    norm_x: f32,
    norm_y: f32,
    tex_width: u32,
    tex_height: u32,
    win_width: u32,
    win_height: u32,
    scale_x: f32,
    scale_y: f32,
    alpha: f32,
}

impl SpriteUniform {
    /// 从归一化坐标和纹理/窗口尺寸计算 uniform 数据。
    ///
    /// # 坐标转换
    /// 归一化坐标 → NDC：
    /// - NDC x = norm_x * 2.0 - 1.0（0→ -1, 0.5→0, 1→1）
    /// - NDC y = (1.0 - norm_y) * 2.0 - 1.0（翻转 Y：0→1, 0.5→0, 1→-1）
    ///
    /// 半尺寸计算：基于纹理像素尺寸和窗口像素尺寸的比例，
    /// 保证立绘在屏幕上不拉伸变形。
    fn compute(p: &UniformParams) -> Self {
        // 步骤 1：归一化坐标 → NDC 坐标
        let ndc_x = p.norm_x * 2.0 - 1.0;
        let ndc_y = (1.0 - p.norm_y) * 2.0 - 1.0;

        // 步骤 2：计算 NDC 半尺寸
        let texture_aspect = p.tex_width as f32 / p.tex_height as f32;
        let window_aspect = p.win_width as f32 / p.win_height as f32;

        let ndc_half_height = (p.tex_height as f32 / p.win_height as f32) * p.scale_y;
        let ndc_half_width = ndc_half_height * texture_aspect / window_aspect * p.scale_x;

        Self {
            sprite_pos: [ndc_x, ndc_y],
            sprite_half_size: [ndc_half_width, ndc_half_height],
            alpha: p.alpha,
            _padding: 0.0,
        }
    }
}

// ============================================================================
// SpriteVertex — 四边形顶点数据
// ============================================================================

/// 立绘四边形顶点 — 包含 NDC 空间位置和纹理坐标。
///
/// 四边形以原点为中心，范围 (-0.5, -0.5) ~ (0.5, 0.5)，
/// 顶点着色器通过 uniform 中的位置和缩放参数将其变换到目标位置。
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SpriteVertex {
    /// 四边形顶点位置（单位坐标，中心在原点）
    position: [f32; 2],
    /// 纹理坐标（已翻转 V 轴以匹配 wgpu 纹理空间）
    uv: [f32; 2],
}

// 单位四边形的 4 个顶点。
//
// 顶点顺序（逆时针，从右下角开始以匹配三角形索引）：
//   3──2    UV 映射：
//   │╲│     顶点0: ( 0.5, -0.5) UV(1,1) - 右下
//   0──1     顶点1: (-0.5, -0.5) UV(0,1) - 左下
//            顶点2: (-0.5,  0.5) UV(0,0) - 左上
//            顶点3: ( 0.5,  0.5) UV(1,0) - 右上
//
// UV 已预翻转 V 轴：wgpu 纹理空间 Y 向下（(0,0)=左上），
// NDC 空间 Y 向上（(-1,-1)=左下），因此 UV V 分量需要翻转。
const QUAD_VERTICES: [SpriteVertex; 4] = [
    // 右下角 — NDC 右下 → 纹理右下
    SpriteVertex {
        position: [0.5, -0.5],
        uv: [1.0, 1.0],
    },
    // 左下角 — NDC 左下 → 纹理左下
    SpriteVertex {
        position: [-0.5, -0.5],
        uv: [0.0, 1.0],
    },
    // 左上角 — NDC 左上 → 纹理左上
    SpriteVertex {
        position: [-0.5, 0.5],
        uv: [0.0, 0.0],
    },
    // 右上角 — NDC 右上 → 纹理右上
    SpriteVertex {
        position: [0.5, 0.5],
        uv: [1.0, 0.0],
    },
];

/// 四边形索引缓冲 — 2 个三角形组成四边形。
///
/// 三角形 1: 0→1→2（右下→左下→左上）
/// 三角形 2: 0→2→3（右下→左上→右上）
const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

// ============================================================================
// SpriteEntry — 立绘内部条目（含 GPU 资源）
// ============================================================================

/// 立绘内部条目 — 包含立绘数据和 GPU 资源。
///
/// 每个立绘持有独立的 uniform 缓冲区和绑定组，
/// 允许在同一帧中快速切换不同立绘的渲染参数。
struct SpriteEntry {
    /// 立绘数据
    sprite: Sprite,
    /// GPU 纹理（所有权在此，支持不同立绘使用不同纹理）
    texture: Texture,
    /// 该立绘的 uniform 缓冲区
    uniform_buffer: Buffer,
    /// 该立绘的 uniform 绑定组
    uniform_bind_group: BindGroup,
}

// ============================================================================
// SpriteLayer — 立绘图层渲染器
// ============================================================================

/// 立绘图层渲染器 — 管理 Layer 1 或 Layer 2 的多个立绘渲染。
///
/// 支持添加/移除/更新立绘，按 z-index 排序渲染。
/// 每个立绘可独立设置位置、缩放、透明度和 z-index。
///
/// # 渲染管线
/// - 图元拓扑：`TriangleList`（2 个三角形组成的四边形）
/// - 颜色混合：Alpha Blending（`SrcAlpha` / `OneMinusSrcAlpha`）
/// - 顶点格式：`Float32x2` position + `Float32x2` uv
/// - 纹理格式：`Rgba8UnormSrgb`（与 Texture 格式一致）
///
/// # 使用示例
/// ```rust,ignore
/// use aster_renderer::{SpriteLayer, SpriteDescriptor, SpritePosition, Texture};
///
/// // 创建立绘层
/// let mut sprite_layer = SpriteLayer::new(
///     ctx.device(), ctx.queue(),
///     ctx.surface_config().format,
///     1920, 1080,
/// );
///
/// // 加载立绘纹理
/// let char_tex = Texture::from_file(
///     ctx.device(), ctx.queue(),
///     "assets/sprites/sayori/default.png", Some("小百合-默认"),
/// )?;
///
/// // 添加立绘
/// let desc = SpriteDescriptor::new(SpritePosition::Left)
///     .with_alpha(1.0)
///     .with_z_index(0);
/// sprite_layer.add_sprite(ctx.device(), ctx.queue(), char_tex, desc);
///
/// // 渲染（通过 LayerManager 调用）
/// ```
pub struct SpriteLayer {
    /// 所有活跃的立绘条目（存储 GPU 资源）
    entries: Vec<SpriteEntry>,
    /// 渲染管线（所有立绘共享同一管线）
    pipeline: RenderPipeline,
    /// 四边形顶点缓冲（所有立绘共享同一几何体）
    vertex_buffer: Buffer,
    /// 四边形索引缓冲
    index_buffer: Buffer,
    /// 索引数量（用于 draw_indexed）
    index_count: u32,
    /// uniform 绑定组布局（每个立绘的 bind_group 使用此布局）
    uniform_bind_group_layout: BindGroupLayout,
    /// 当前窗口宽度（用于 uniform 坐标计算）
    window_width: u32,
    /// 当前窗口高度
    window_height: u32,
}

impl SpriteLayer {
    /// 创建立绘图层渲染器。
    ///
    /// 初始化渲染管线、顶点/索引缓冲区和 uniform 绑定组布局。
    /// 初始状态无立绘，渲染时跳过绘制。
    ///
    /// # 参数
    /// - `device`: wgpu 设备引用
    /// - `queue`: wgpu 命令队列引用
    /// - `format`: 颜色纹理格式（必须与 surface 配置一致）
    /// - `width`: 窗口宽度（逻辑像素）
    /// - `height`: 窗口高度（逻辑像素）
    ///
    /// # 内部流程
    /// 1. 创建顶点缓冲（4 顶点四边形）和索引缓冲（6 索引）
    /// 2. 创建纹理绑定组布局（@group(0)，复用 crate::texture 的共享布局）
    /// 3. 创建 uniform 绑定组布局（@group(1): SpriteUniform）
    /// 4. 创建管线布局 + 渲染管线（TriangleList + AlphaBlending）
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        // 步骤 1：创建顶点缓冲
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("立绘顶点缓冲"),
            size: (mem::size_of::<SpriteVertex>() * QUAD_VERTICES.len()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&QUAD_VERTICES));

        // 步骤 2：创建索引缓冲
        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("立绘索引缓冲"),
            size: (mem::size_of::<u16>() * QUAD_INDICES.len()) as u64,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&QUAD_INDICES));

        let index_count = QUAD_INDICES.len() as u32;

        // 步骤 3：创建纹理绑定组布局（@group(0) — 复用共享布局）
        let texture_bind_group_layout = crate::texture::create_texture_bind_group_layout(device);

        // 步骤 4：创建 uniform 绑定组布局（@group(1): SpriteUniform）
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("立绘 Uniform 绑定组布局"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // 步骤 5：创建管线布局
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("立绘层管线布局"),
            bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 步骤 6：编译着色器并创建渲染管线
        let shader_module = device.create_shader_module(include_wgsl!("shaders/sprite.wgsl"));

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("立绘层渲染管线"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: mem::size_of::<SpriteVertex>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        // @location(0): position (vec2<f32>)
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        // @location(1): uv (vec2<f32>)
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: mem::size_of::<[f32; 2]>() as u64,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format,
                    // Alpha 混合：src_alpha * src + (1 - src_alpha) * dst
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                    }),
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

        Self {
            entries: Vec::new(),
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            uniform_bind_group_layout,
            window_width: width.max(1),
            window_height: height.max(1),
        }
    }

    // ========================================================================
    // 立绘管理 API
    // ========================================================================

    /// 添加立绘到图层。
    ///
    /// 将纹理和描述符组合创建完整的立绘条目（含 GPU uniform 资源）。
    /// 如果已有相同 ID 的立绘，旧立绘被替换。
    ///
    /// # 参数
    /// - `device`: wgpu 设备引用
    /// - `queue`: wgpu 命令队列引用
    /// - `texture`: 立绘纹理（所有权转移）
    /// - `desc`: 立绘配置描述符
    ///
    /// # 返回值
    /// - 新创建立绘的唯一 ID（用于后续 `remove_sprite` / `update_*` 操作）
    ///
    /// # 内部流程
    /// 1. 分配唯一 sprite ID
    /// 2. 根据纹理尺寸和窗口尺寸计算 SpriteUniform
    /// 3. 创建 uniform 缓冲区并上传初始数据
    /// 4. 创建该立绘的 uniform 绑定组
    /// 5. 将条目加入内部列表
    pub fn add_sprite(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture: Texture,
        desc: SpriteDescriptor,
    ) -> u64 {
        let sprite_id = next_sprite_id();
        let texture_id = texture.id;
        let (norm_x, norm_y) = desc.position.to_coords();

        // 创建该立绘的 Uniform
        let uniform = SpriteUniform::compute(&UniformParams {
            norm_x,
            norm_y,
            tex_width: texture.width,
            tex_height: texture.height,
            win_width: self.window_width,
            win_height: self.window_height,
            scale_x: desc.scale.0,
            scale_y: desc.scale.1,
            alpha: desc.alpha,
        });

        // 创建 uniform 缓冲区
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("立绘 Uniform 缓冲 #{sprite_id}")),
            size: mem::size_of::<SpriteUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        // 创建 uniform 绑定组
        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("立绘 Uniform 绑定组 #{sprite_id}")),
            layout: &self.uniform_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(uniform_buffer.as_entire_buffer_binding()),
            }],
        });

        let sprite = Sprite {
            id: sprite_id,
            texture_id,
            position: desc.position,
            scale: desc.scale,
            alpha: desc.alpha,
            z_index: desc.z_index,
        };

        // 如果已有相同 ID 的条目，替换之
        if let Some(pos) = self.entries.iter().position(|e| e.sprite.id == sprite_id) {
            self.entries.remove(pos);
        }

        self.entries.push(SpriteEntry {
            sprite,
            texture,
            uniform_buffer,
            uniform_bind_group,
        });

        sprite_id
    }

    /// 移除指定 ID 的立绘。
    ///
    /// 如果该 ID 不存在，静默忽略。
    ///
    /// # 参数
    /// - `sprite_id`: 由 `add_sprite()` 返回的立绘 ID
    ///
    /// # 返回值
    /// - `true`: 成功移除
    /// - `false`: 未找到该 ID 的立绘
    pub fn remove_sprite(&mut self, sprite_id: u64) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.sprite.id != sprite_id);
        self.entries.len() < len_before
    }

    /// 更新指定立绘的屏幕位置。
    ///
    /// 重新计算 NDC 坐标并更新 uniform 缓冲区。
    /// 如果该 ID 不存在，静默忽略。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `sprite_id`: 立绘 ID
    /// - `position`: 新的屏幕位置
    pub fn update_position(&mut self, queue: &Queue, sprite_id: u64, position: SpritePosition) {
        let Some(entry) = self.entries.iter_mut().find(|e| e.sprite.id == sprite_id) else {
            return;
        };

        entry.sprite.position = position;
        let w = self.window_width;
        let h = self.window_height;
        Self::write_uniform_for_entry(queue, entry, w, h);
    }

    /// 更新指定立绘的透明度。
    ///
    /// alpha 自动 clamp 到 [0.0, 1.0]。
    /// 如果该 ID 不存在，静默忽略。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `sprite_id`: 立绘 ID
    /// - `alpha`: 新的透明度值
    pub fn update_alpha(&mut self, queue: &Queue, sprite_id: u64, alpha: f32) {
        let Some(entry) = self.entries.iter_mut().find(|e| e.sprite.id == sprite_id) else {
            return;
        };

        entry.sprite.alpha = alpha.clamp(0.0, 1.0);
        let w = self.window_width;
        let h = self.window_height;
        Self::write_uniform_for_entry(queue, entry, w, h);
    }

    /// 更新指定立绘的缩放因子。
    ///
    /// 如果该 ID 不存在，静默忽略。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `sprite_id`: 立绘 ID
    /// - `scale_x`: 水平缩放因子
    /// - `scale_y`: 垂直缩放因子
    pub fn update_scale(&mut self, queue: &Queue, sprite_id: u64, scale_x: f32, scale_y: f32) {
        let Some(entry) = self.entries.iter_mut().find(|e| e.sprite.id == sprite_id) else {
            return;
        };

        entry.sprite.scale = (scale_x, scale_y);
        let w = self.window_width;
        let h = self.window_height;
        Self::write_uniform_for_entry(queue, entry, w, h);
    }

    /// 清除所有立绘，释放 GPU 资源。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 返回当前活跃立绘数量。
    #[inline]
    #[must_use]
    pub fn sprite_count(&self) -> usize {
        self.entries.len()
    }

    /// 获取指定 ID 的立绘数据（不含 GPU 资源）。
    ///
    /// # 参数
    /// - `sprite_id`: 立绘 ID
    ///
    /// # 返回值
    /// - `Some(&Sprite)`: 立绘数据引用
    /// - `None`: 未找到
    #[must_use]
    pub fn get_sprite(&self, sprite_id: u64) -> Option<&Sprite> {
        self.entries
            .iter()
            .find(|e| e.sprite.id == sprite_id)
            .map(|e| &e.sprite)
    }

    /// 返回所有立绘数据的迭代器。
    pub fn sprites(&self) -> impl Iterator<Item = &Sprite> {
        self.entries.iter().map(|e| &e.sprite)
    }

    // ========================================================================
    // 窗口 resize
    // ========================================================================

    /// 响应窗口 resize 事件，更新所有立绘的 uniform 数据。
    ///
    /// 窗口尺寸变化会导致 NDC 半尺寸变化（因为它是基于像素比例计算的），
    /// 因此需要重新计算并上传所有立绘的 uniform。
    ///
    /// # 参数
    /// - `queue`: wgpu 命令队列引用
    /// - `width`: 新的窗口宽度（自动 clamp 到 ≥1）
    /// - `height`: 新的窗口高度（自动 clamp 到 ≥1）
    pub fn resize(&mut self, queue: &Queue, width: u32, height: u32) {
        self.window_width = width.max(1);
        self.window_height = height.max(1);

        // 重新计算并上传所有立绘的 uniform
        let w = self.window_width;
        let h = self.window_height;
        for entry in &self.entries {
            Self::write_uniform_for_entry(queue, entry, w, h);
        }
    }

    // ========================================================================
    // 私有辅助方法
    // ========================================================================

    /// 为立绘条目计算并写入 uniform 数据到其 GPU 缓冲区。
    fn write_uniform_for_entry(
        queue: &Queue,
        entry: &SpriteEntry,
        win_width: u32,
        win_height: u32,
    ) {
        let (norm_x, norm_y) = entry.sprite.position.to_coords();

        let uniform = SpriteUniform::compute(&UniformParams {
            norm_x,
            norm_y,
            tex_width: entry.texture.width,
            tex_height: entry.texture.height,
            win_width,
            win_height,
            scale_x: entry.sprite.scale.0,
            scale_y: entry.sprite.scale.1,
            alpha: entry.sprite.alpha,
        });

        queue.write_buffer(&entry.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }
}

// ============================================================================
// Layer trait 实现 — 将 SpriteLayer 接入 LayerManager
// ============================================================================

impl Layer for SpriteLayer {
    fn render<'a>(&'a self, encoder: &'a mut CommandEncoder, output_view: &'a TextureView) {
        // 无立绘时静默跳过
        if self.entries.is_empty() {
            return;
        }

        // 按 z_index 升序排序索引（数值小的先渲染，在底层）
        let mut sorted_indices: Vec<usize> = (0..self.entries.len()).collect();
        sorted_indices.sort_by_key(|&i| self.entries[i].sprite.z_index);

        // 创建 render pass（Load = 保留已有内容）
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("立绘层渲染 Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // 保留背景层内容
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);

        // 逐立绘渲染
        for &idx in &sorted_indices {
            let entry = &self.entries[idx];

            // @group(0): 纹理 + 采样器
            render_pass.set_bind_group(0, &entry.texture.bind_group, &[]);
            // @group(1): 逐立绘 uniform
            render_pass.set_bind_group(1, &entry.uniform_bind_group, &[]);

            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
        // render_pass 在此 drop，结束渲染通道
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01, AC03, AC05
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    // ========================================================================
    // 测试辅助函数
    // ========================================================================

    /// 生成 1×1 红色 PNG 字节。
    fn make_test_png(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(1, 1, Rgba([r, g, b, a]));
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
                label: Some("立绘层测试设备"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        Some((device, queue))
    }

    /// 创建测试用输出纹理视图。
    fn create_test_output_view(device: &Device, width: u32, height: u32) -> TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("立绘层测试输出"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// 加载测试纹理。
    fn load_test_texture(device: &Device, queue: &Queue, r: u8, g: u8, b: u8, a: u8) -> Texture {
        let bytes = make_test_png(r, g, b, a);
        Texture::from_bytes(device, queue, &bytes, Some("测试立绘纹理")).expect("测试纹理加载失败")
    }

    // ========================================================================
    // AC03 — 立绘位置枚举映射正确
    // ========================================================================

    /// AC03: 验证 Left/Center/Right 的归一化坐标映射。
    #[test]
    fn ac03_position_left_is_025_05() {
        assert_eq!(SpritePosition::Left.to_coords(), (0.25, 0.5));
    }

    /// AC03: Center → (0.5, 0.5)。
    #[test]
    fn ac03_position_center_is_05_05() {
        assert_eq!(SpritePosition::Center.to_coords(), (0.5, 0.5));
    }

    /// AC03: Right → (0.75, 0.5)。
    #[test]
    fn ac03_position_right_is_075_05() {
        assert_eq!(SpritePosition::Right.to_coords(), (0.75, 0.5));
    }

    /// AC03: Custom 直接返回传入的坐标。
    #[test]
    fn ac03_position_custom_passthrough() {
        assert_eq!(SpritePosition::Custom(0.1, 0.9).to_coords(), (0.1, 0.9));
        assert_eq!(SpritePosition::Custom(0.0, 1.0).to_coords(), (0.0, 1.0));
        assert_eq!(SpritePosition::Custom(1.0, 0.0).to_coords(), (1.0, 0.0));
    }

    /// AC03: Default 为 Center。
    #[test]
    fn ac03_position_default_is_center() {
        assert_eq!(SpritePosition::default(), SpritePosition::Center);
    }

    // ========================================================================
    // AC01 — 单个立绘正确渲染（不 panic）
    // ========================================================================

    /// AC01: 验证创建 SpriteLayer 并添加单个立绘后渲染不 panic。
    #[test]
    fn ac01_single_sprite_render_no_crash() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC01 单立绘测试");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        assert_eq!(layer.sprite_count(), 0);

        // 添加立绘
        let tex = load_test_texture(&device, &queue, 255, 128, 64, 255);
        let desc = SpriteDescriptor::new(SpritePosition::Center);
        let sprite_id = layer.add_sprite(&device, &queue, tex, desc);

        assert_eq!(layer.sprite_count(), 1);
        assert!(layer.get_sprite(sprite_id).is_some());

        // 渲染
        let output_view = create_test_output_view(&device, 1920, 1080);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC01 渲染测试"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    /// AC01: 验证空立绘层渲染不 panic。
    #[test]
    fn ac01_empty_sprite_layer_render_no_crash() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC01 空层测试");
                return;
            }
        };

        let layer = SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        let output_view = create_test_output_view(&device, 1920, 1080);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC01 空层渲染"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    // ========================================================================
    // AC05 — clear() 后所有立绘被移除
    // ========================================================================

    /// AC05: 验证 clear() 清除所有立绘。
    #[test]
    fn ac05_clear_removes_all_sprites() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC05 clear 测试");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 添加 3 个立绘
        for i in 0..3 {
            let tex = load_test_texture(&device, &queue, (i * 80) as u8, 128, 255, 255);
            let desc = SpriteDescriptor::new(SpritePosition::Custom(i as f32 * 0.3 + 0.1, 0.5));
            layer.add_sprite(&device, &queue, tex, desc);
        }

        assert_eq!(layer.sprite_count(), 3);

        // 清除
        layer.clear();
        assert_eq!(layer.sprite_count(), 0);

        // 清除后渲染不 panic
        let output_view = create_test_output_view(&device, 1920, 1080);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC05 clear 后渲染"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    /// AC05: 验证空列表 clear 不 panic。
    #[test]
    fn ac05_clear_empty_no_panic() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC05 空 clear 测试");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        layer.clear(); // 对空列表 clear 不应 panic
        assert_eq!(layer.sprite_count(), 0);
    }

    // ========================================================================
    // AC04 — 透明度更新
    // ========================================================================

    /// AC04: 验证 update_alpha 正确更新立绘透明度和 uniform。
    #[test]
    fn ac04_update_alpha_changes_sprite_alpha() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC04 alpha 测试");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        let tex = load_test_texture(&device, &queue, 255, 0, 0, 255);
        let desc = SpriteDescriptor::new(SpritePosition::Center).with_alpha(1.0);
        let sprite_id = layer.add_sprite(&device, &queue, tex, desc);

        assert_eq!(layer.get_sprite(sprite_id).unwrap().alpha, 1.0);

        // 更新透明度为 0.5
        layer.update_alpha(&queue, sprite_id, 0.5);
        assert_eq!(layer.get_sprite(sprite_id).unwrap().alpha, 0.5);

        // 透明度 clamp 测试
        layer.update_alpha(&queue, sprite_id, 2.5);
        assert_eq!(layer.get_sprite(sprite_id).unwrap().alpha, 1.0);

        layer.update_alpha(&queue, sprite_id, -0.5);
        assert_eq!(layer.get_sprite(sprite_id).unwrap().alpha, 0.0);

        // 渲染验证不 panic
        let output_view = create_test_output_view(&device, 1920, 1080);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC04 alpha 渲染"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    // ========================================================================
    // AC02 — z-index 排序验证
    // ========================================================================

    /// AC02: 验证多个不同 z-index 的立绘在渲染时按 z-index 排序。
    #[test]
    fn ac02_z_index_sorting() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC02 z-index 测试");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 按非排序顺序添加（z=10, z=0, z=5）
        let tex0 = load_test_texture(&device, &queue, 255, 0, 0, 255);
        let id0 = layer.add_sprite(
            &device,
            &queue,
            tex0,
            SpriteDescriptor::new(SpritePosition::Left).with_z_index(10),
        );

        let tex1 = load_test_texture(&device, &queue, 0, 255, 0, 255);
        let id1 = layer.add_sprite(
            &device,
            &queue,
            tex1,
            SpriteDescriptor::new(SpritePosition::Center).with_z_index(0),
        );

        let tex2 = load_test_texture(&device, &queue, 0, 0, 255, 255);
        let id2 = layer.add_sprite(
            &device,
            &queue,
            tex2,
            SpriteDescriptor::new(SpritePosition::Right).with_z_index(5),
        );

        assert_eq!(layer.sprite_count(), 3);

        // 验证 z_index 值已正确存储
        assert_eq!(layer.get_sprite(id0).unwrap().z_index, 10);
        assert_eq!(layer.get_sprite(id1).unwrap().z_index, 0);
        assert_eq!(layer.get_sprite(id2).unwrap().z_index, 5);

        // 渲染验证不 panic（z-index 排序由 render 内部处理）
        let output_view = create_test_output_view(&device, 1920, 1080);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("AC02 z-index 渲染"),
        });
        layer.render(&mut encoder, &output_view);
        encoder.finish();
    }

    // ========================================================================
    // 其他功能测试
    // ========================================================================

    /// 验证 remove_sprite 正确移除指定立绘。
    #[test]
    fn remove_sprite_by_id() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        let tex = load_test_texture(&device, &queue, 255, 0, 0, 255);
        let id = layer.add_sprite(
            &device,
            &queue,
            tex,
            SpriteDescriptor::new(SpritePosition::Center),
        );

        assert_eq!(layer.sprite_count(), 1);

        // 移除不存在的 ID
        assert!(!layer.remove_sprite(99999));
        assert_eq!(layer.sprite_count(), 1);

        // 移除存在的 ID
        assert!(layer.remove_sprite(id));
        assert_eq!(layer.sprite_count(), 0);
    }

    /// 验证 update_position 正确更新位置。
    #[test]
    fn update_position_changes_sprite_position() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        let tex = load_test_texture(&device, &queue, 255, 0, 0, 255);
        let id = layer.add_sprite(
            &device,
            &queue,
            tex,
            SpriteDescriptor::new(SpritePosition::Left),
        );

        assert_eq!(layer.get_sprite(id).unwrap().position, SpritePosition::Left);

        // 更新位置到 Right
        layer.update_position(&queue, id, SpritePosition::Right);
        assert_eq!(
            layer.get_sprite(id).unwrap().position,
            SpritePosition::Right
        );

        // 更新到 Custom
        layer.update_position(&queue, id, SpritePosition::Custom(0.1, 0.9));
        assert_eq!(
            layer.get_sprite(id).unwrap().position,
            SpritePosition::Custom(0.1, 0.9)
        );
    }

    /// 验证 update_position / update_alpha 对不存在的 ID 不 panic。
    #[test]
    fn update_nonexistent_sprite_no_panic() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        // 对空层更新不应 panic
        layer.update_position(&queue, 42, SpritePosition::Center);
        layer.update_alpha(&queue, 42, 0.5);
        layer.update_scale(&queue, 42, 1.0, 1.0);
    }

    /// 验证 sprites() 迭代器正确返回所有立绘。
    #[test]
    fn sprites_iterator_returns_all() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        let tex1 = load_test_texture(&device, &queue, 255, 0, 0, 255);
        let tex2 = load_test_texture(&device, &queue, 0, 255, 0, 255);

        layer.add_sprite(&device, &queue, tex1, SpriteDescriptor::default());
        layer.add_sprite(&device, &queue, tex2, SpriteDescriptor::default());

        let count = layer.sprites().count();
        assert_eq!(count, 2);
    }

    /// 验证 SpriteUniform 内存布局。
    #[test]
    fn sprite_uniform_layout() {
        let uniform = SpriteUniform::compute(&UniformParams {
            norm_x: 0.25,
            norm_y: 0.5,
            tex_width: 1080,
            tex_height: 1920,
            win_width: 1920,
            win_height: 1080,
            scale_x: 1.0,
            scale_y: 1.0,
            alpha: 1.0,
        });
        let bytes = bytemuck::bytes_of(&uniform);

        // 预期大小：24 字节（3 × vec2 + f32 + padding）
        assert_eq!(bytes.len(), 24);

        // 验证可以 round-trip
        let uniform2: &SpriteUniform = bytemuck::from_bytes(bytes);
        assert_eq!(uniform2.sprite_pos, uniform.sprite_pos);
        assert_eq!(uniform2.sprite_half_size, uniform.sprite_half_size);
        assert_eq!(uniform2.alpha, uniform.alpha);
    }

    /// 验证 SpriteVertex 和 SpriteUniform 实现 Pod + Zeroable。
    #[test]
    fn vertex_and_uniform_are_pod_zeroable() {
        let vertex = SpriteVertex {
            position: [0.0, 0.0],
            uv: [0.0, 0.0],
        };
        let _bytes = bytemuck::bytes_of(&vertex);

        let uniform = SpriteUniform {
            sprite_pos: [0.0, 0.0],
            sprite_half_size: [0.1, 0.1],
            alpha: 1.0,
            _padding: 0.0,
        };
        let _bytes = bytemuck::bytes_of(&uniform);
    }

    /// 验证 SpriteDescriptor builder 模式。
    #[test]
    fn sprite_descriptor_builder() {
        let desc = SpriteDescriptor::new(SpritePosition::Left)
            .with_scale(0.8, 0.9)
            .with_alpha(0.5)
            .with_z_index(3);

        assert_eq!(desc.position, SpritePosition::Left);
        assert_eq!(desc.scale, (0.8, 0.9));
        assert_eq!(desc.alpha, 0.5);
        assert_eq!(desc.z_index, 3);
    }

    /// 验证 SpriteDescriptor alpha clamp。
    #[test]
    fn sprite_descriptor_alpha_clamped() {
        // 验证每次 with_alpha 调用都会 clamp 到 [0.0, 1.0]
        let desc_high = SpriteDescriptor::new(SpritePosition::Center).with_alpha(1.5);
        assert_eq!(desc_high.alpha, 1.0, ">1.0 应 clamp 到 1.0");

        let desc_low = SpriteDescriptor::new(SpritePosition::Center).with_alpha(-0.3);
        assert_eq!(desc_low.alpha, 0.0, "<0.0 应 clamp 到 0.0");

        let desc_normal = SpriteDescriptor::new(SpritePosition::Center).with_alpha(0.75);
        assert_eq!(desc_normal.alpha, 0.75, "正常值不应被修改");
    }

    /// 验证 resize 更新窗口尺寸。
    #[test]
    fn resize_updates_dimensions() {
        let (device, queue) = match create_minimal_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut layer =
            SpriteLayer::new(&device, &queue, TextureFormat::Rgba8UnormSrgb, 1920, 1080);

        assert_eq!(layer.window_width, 1920);
        assert_eq!(layer.window_height, 1080);

        layer.resize(&queue, 1280, 720);
        assert_eq!(layer.window_width, 1280);
        assert_eq!(layer.window_height, 720);

        // resize(0, 0) 自动 clamp
        layer.resize(&queue, 0, 0);
        assert_eq!(layer.window_width, 1);
        assert_eq!(layer.window_height, 1);
    }
}
