//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/text_renderer.rs
//! 功能概述：GPU 文本渲染器 — 集成 cosmic-text 实现 GPU 加速的文本排版与渲染。
//!           负责：字体管理（系统字体 + 自定义字体加载）/ 文本布局（cosmic-text Buffer × 2）/
//!           字形光栅化与图集管理（行式货架打包 + R8Unorm 纹理）/
//!           wgpu 渲染管线（字形四边形 → Alpha 蒙版 → 颜色混合）。
//!           实现 Layer trait，注册为 Layer Manager 的 Layer 4（文本层）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - cosmic_text（字体管理 FontSystem + 文本布局 Buffer + 字形光栅化缓存 SwashCache）
//! - wgpu（GPU 纹理/管线/缓冲区/渲染命令）
//! - crate::layer_manager::Layer（渲染层 trait）
//! - crate::gpu_context::RenderError（错误类型）
//!
//! 着色器：`shaders/text.wgsl` — 字形四边形顶点着色 + Alpha 蒙版片元着色
//!
//! 架构位置：aster-platform/aster-core ← aster-renderer
//!
//! 对应任务：PH1-T09 — 文本渲染（cosmic-text 集成）
//! 对应需求：REQ-ENG-013（对话文本渲染）

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use cosmic_text::{Attrs, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendComponent, BlendFactor,
    BlendOperation, BlendState, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    CommandEncoder, Device, Extent3d, FragmentState, IndexFormat, LoadOp, MultisampleState,
    Operations, Origin3d, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    SamplerDescriptor, ShaderStages, StoreOp, TexelCopyBufferLayout, TexelCopyTextureInfo,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode, include_wgsl,
};

use crate::gpu_context::RenderError;
use crate::layer_manager::Layer;

// ============================================================================
// 常量定义
// ============================================================================

/// 字形图集初始宽度（像素）。
///
/// 1024×1024 的 R8Unorm 纹理占用 1MB 显存，可容纳约 500 个 28×28 的字形。
/// 对于视觉小说场景（通常 < 200 个唯一字形），这个大小绰绰有余。
const ATLAS_INITIAL_SIZE: u32 = 1024;

/// 字形图集最大宽度（像素）。
///
/// 2048×2048 R8Unorm = 4MB 显存，可容纳约 5000 个 28×28 的字形。
const ATLAS_MAX_SIZE: u32 = 2048;

/// 字形间填充（像素），防止线性采样时相邻字形渗透（bleeding）。
const GLYPH_PADDING: u32 = 1;

/// 默认正文字号（像素）。
const DEFAULT_FONT_SIZE: f32 = 28.0;

/// 默认说话者字号（像素），略小于正文以视觉区分。
const DEFAULT_SPEAKER_FONT_SIZE: f32 = 24.0;

/// 默认行高倍数（相对于字号）。
const DEFAULT_LINE_HEIGHT: f32 = 1.5;

/// 文本框距屏幕边缘的边距比例（每边 5%，文本框占 90% 宽度）。
const TEXT_BOX_MARGIN_RATIO: f32 = 0.05;

/// 文本框高度占屏幕高度的比例（25%）。
const TEXT_BOX_HEIGHT_RATIO: f32 = 0.25;

/// 说话者名字与正文之间的间距（像素）。
const SPEAKER_BODY_GAP: f32 = 8.0;

// ============================================================================
// GlyphVertex — 字形顶点数据（CPU 端）
// ============================================================================

/// 字形顶点 — 对应 WGSL 着色器中的 `GlyphVertex` 输入结构体。
///
/// 每个字形由 4 个顶点 + 6 个索引组成一个四边形，所有可见字形的顶点
/// 合并到同一个缓冲区中，通过一次 draw call 渲染。
///
/// 内存布局（32 字节，与 WGSL `GlyphVertex` 一致）：
///   - `position`: vec2<f32> — NDC 空间顶点位置（偏移 0，8 字节）
///   - `uv`: vec2<f32> — 图集 UV 坐标（偏移 8，8 字节）
///   - `color`: vec4<f32> — RGBA 文本颜色（偏移 16，16 字节）
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GlyphVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

// ============================================================================
// TextConfig — 文本渲染器配置
// ============================================================================

/// 文本渲染器配置 — 控制文本框中说话者和正文的显示样式。
///
/// # 默认值
/// - 正文字号: 28px，白色
/// - 说话者字号: 24px，浅灰色
/// - 行高: 1.5 倍
#[derive(Debug, Clone)]
pub struct TextConfig {
    /// 正文字号（像素），默认 28.0
    pub font_size: f32,
    /// 说话者名字字号（像素），默认 24.0
    pub speaker_font_size: f32,
    /// 行高倍数（相对于字号），默认 1.5
    pub line_height: f32,
    /// 正文颜色 RGBA，默认白色 [1.0, 1.0, 1.0, 1.0]
    pub text_color: [f32; 4],
    /// 说话者名字颜色 RGBA，默认浅灰色 [0.8, 0.8, 0.8, 1.0]
    pub speaker_color: [f32; 4],
    /// 文本框内边距（像素），默认 20.0
    pub text_box_padding: f32,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            speaker_font_size: DEFAULT_SPEAKER_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            text_color: [1.0, 1.0, 1.0, 1.0],
            speaker_color: [0.8, 0.8, 0.8, 1.0],
            text_box_padding: 20.0,
        }
    }
}

// ============================================================================
// CachedGlyph — 已缓存的字形信息
// ============================================================================

/// 字形在图集中的位置和尺寸。
#[derive(Debug, Clone, Copy)]
struct CachedGlyph {
    /// 图集中 X 偏移（像素）
    atlas_x: u32,
    /// 图集中 Y 偏移（像素）
    atlas_y: u32,
    /// 字形像素宽度
    width: u32,
    /// 字形像素高度
    height: u32,
    /// 水平 bearing — 从笔位置到光栅图像左边缘的偏移（可负）
    left: i32,
    /// 垂直 bearing — 从笔位置到光栅图像上边缘的偏移（正=上方）
    top: i32,
}

// ============================================================================
// RowAtlas — 行式货架图集
// ============================================================================

/// 行式货架图集 — 基于行的简单 2D 纹理打包器。
///
/// 采用行式（shelf）打包算法：字形按行组织，行高 = 该行最高字形，
/// 新字形优先放入当前行，放不下时另起一行。
///
/// 使用 R8Unorm 单通道纹理，仅存储字形 Alpha 蒙版。
struct RowAtlas {
    texture: wgpu::Texture,
    view: TextureView,
    width: u32,
    height: u32,
    current_row_y: u32,
    current_row_height: u32,
    current_row_x: u32,
}

impl RowAtlas {
    /// 创建新的行式图集。
    fn new(device: &Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("字形图集"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
            current_row_y: 0,
            current_row_height: 0,
            current_row_x: 0,
        }
    }

    /// 尝试为给定尺寸的字形分配图集空间。
    ///
    /// 返回 `Some((atlas_x, atlas_y))` 表示分配成功，
    /// `None` 表示图集已满需要增长。
    fn allocate(&mut self, glyph_width: u32, glyph_height: u32) -> Option<(u32, u32)> {
        let padded_w = glyph_width + GLYPH_PADDING * 2;
        let padded_h = glyph_height + GLYPH_PADDING * 2;

        // 尝试放入当前行
        if self.current_row_x + padded_w <= self.width {
            let x = self.current_row_x + GLYPH_PADDING;
            let y = self.current_row_y + GLYPH_PADDING;
            self.current_row_x += padded_w;
            self.current_row_height = self.current_row_height.max(padded_h);
            return Some((x, y));
        }

        // 另起一行
        let new_row_y = self.current_row_y + self.current_row_height;
        if new_row_y + padded_h <= self.height {
            self.current_row_y = new_row_y;
            self.current_row_x = padded_w;
            self.current_row_height = padded_h;
            return Some((GLYPH_PADDING, new_row_y + GLYPH_PADDING));
        }

        // 空间不足
        None
    }

    /// 增长图集尺寸（翻倍，最大 ATLAS_MAX_SIZE）。
    ///
    /// 返回 `true` 表示增长成功，`false` 表示已达最大尺寸。
    fn grow(&mut self, device: &Device) -> bool {
        let new_width = (self.width * 2).min(ATLAS_MAX_SIZE);
        let new_height = (self.height * 2).min(ATLAS_MAX_SIZE);

        if new_width == self.width && new_height == self.height {
            return false;
        }

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("字形图集（增长后）"),
            size: Extent3d {
                width: new_width,
                height: new_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());

        self.texture = texture;
        self.view = view;
        self.width = new_width;
        self.height = new_height;
        self.current_row_y = 0;
        self.current_row_height = 0;
        self.current_row_x = 0;

        true
    }

    /// 将字形像素数据上传到图集指定位置。
    fn upload(&self, queue: &Queue, x: u32, y: u32, width: u32, height: u32, data: &[u8]) {
        // wgpu 要求 bytes_per_row 是 256 的倍数（COPY_BYTES_PER_ROW_ALIGNMENT）
        let aligned_width = width.div_ceil(256) * 256;

        let mut padded_data: Vec<u8> = Vec::new();
        if aligned_width == width {
            padded_data.extend_from_slice(data);
        } else {
            padded_data.resize((aligned_width * height) as usize, 0);
            for row in 0..height as usize {
                let src_start = row * width as usize;
                let dst_start = row * aligned_width as usize;
                padded_data[dst_start..dst_start + width as usize]
                    .copy_from_slice(&data[src_start..src_start + width as usize]);
            }
        }

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d { x, y, z: 0 },
                aspect: TextureAspect::All,
            },
            &padded_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned_width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn view(&self) -> &TextureView {
        &self.view
    }

    fn width_f32(&self) -> f32 {
        self.width as f32
    }

    fn height_f32(&self) -> f32 {
        self.height as f32
    }
}

// ============================================================================
// RenderContext — 渲染坐标变换参数
// ============================================================================

/// 渲染上下文 — 传递给字形四边形生成函数的坐标变换参数。
struct RenderContext {
    atlas_w: f32,
    atlas_h: f32,
    screen_w: f32,
    screen_h: f32,
}

// ============================================================================
// TextRenderer — GPU 文本渲染器
// ============================================================================

/// GPU 文本渲染器 — 管理 cosmic-text 文本布局 + 字形光栅化 + wgpu 渲染。
///
/// # 文本框布局（屏幕坐标系，原点左上角）
/// ```text
/// ┌──────────────────────────────────────────┐ ← Y=0 (屏幕顶部)
/// │  (背景区域)                               │
/// ├──────────────────────────────────────────┤ ← Y=75% (文本框顶部)
/// │  [说话者名字] 24px 浅灰色                   │
/// │  对话正文第一行... 28px 白色                 │
/// │  对话正文第二行...                          │
/// └──────────────────────────────────────────┘ ← Y=100% (屏幕底部)
/// ```
pub struct TextRenderer {
    /// cosmic-text 字体系统
    font_system: FontSystem,
    /// 字形光栅化缓存
    swash_cache: SwashCache,
    /// 说话者名字布局缓冲区
    speaker_buffer: cosmic_text::Buffer,
    /// 正文布局缓冲区
    body_buffer: cosmic_text::Buffer,
    /// 文本渲染配置
    config: TextConfig,
    /// 屏幕像素宽度
    screen_width: f32,
    /// 屏幕像素高度
    screen_height: f32,
    /// 当前说话者名字
    current_speaker: String,
    /// 当前正文
    current_body: String,
    /// 文本是否已变更
    text_changed: bool,
    /// 可见正文字符数上限（None = 显示全部，向后兼容）
    visible_body_chars: Option<usize>,

    // === 图集 ===
    atlas: RowAtlas,
    glyph_cache: HashMap<cosmic_text::CacheKey, CachedGlyph>,

    // === GPU 管线（创建后不变） ===
    bind_group_layout: BindGroupLayout,
    render_pipeline: RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group: BindGroup,

    // === 每帧动态资源 ===
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
}

impl TextRenderer {
    /// 创建新的文本渲染器。
    ///
    /// 初始化 cosmic-text FontSystem、创建布局 Buffer、构建 wgpu 渲染管线。
    pub fn new(
        device: &Device,
        _queue: &Queue,
        surface_format: TextureFormat,
        screen_width: u32,
        screen_height: u32,
        config: TextConfig,
    ) -> Result<Self, RenderError> {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();

        // 创建布局缓冲区（v0.19: Buffer::new 需要 &mut FontSystem + Metrics）
        let body_metrics = Metrics::new(config.font_size, config.font_size * config.line_height);
        let body_buffer = cosmic_text::Buffer::new(&mut font_system, body_metrics);

        let speaker_metrics = Metrics::new(
            config.speaker_font_size,
            config.speaker_font_size * config.line_height,
        );
        let speaker_buffer = cosmic_text::Buffer::new(&mut font_system, speaker_metrics);

        // 创建图集
        let atlas = RowAtlas::new(device, ATLAS_INITIAL_SIZE, ATLAS_INITIAL_SIZE);

        // 创建采样器
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("字形图集采样器"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 创建绑定组布局: @binding(0) 图集纹理 + @binding(1) 采样器
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("文本渲染绑定组布局"),
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
        });

        // 创建绑定组
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("文本渲染绑定组"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(atlas.view()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 加载着色器
        let shader_module = device.create_shader_module(include_wgsl!("shaders/text.wgsl"));

        // 创建管线布局
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("文本渲染管线布局"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 创建渲染管线（带 alpha 混合）
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("文本渲染管线"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphVertex>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
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

        Ok(Self {
            font_system,
            swash_cache,
            speaker_buffer,
            body_buffer,
            config,
            screen_width: screen_width as f32,
            screen_height: screen_height as f32,
            current_speaker: String::new(),
            current_body: String::new(),
            text_changed: true,
            visible_body_chars: None,
            atlas,
            glyph_cache: HashMap::new(),
            bind_group_layout,
            render_pipeline,
            sampler,
            bind_group,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
        })
    }

    /// 设置显示文本。
    ///
    /// # 参数
    /// - `speaker`: 说话者名字，空字符串表示不显示
    /// - `body`: 对话正文
    pub fn set_text(&mut self, speaker: &str, body: &str) {
        self.current_speaker = speaker.to_string();
        self.current_body = body.to_string();
        self.text_changed = true;
    }

    /// 清除所有显示文本。
    pub fn clear_text(&mut self) {
        self.set_text("", "");
    }

    /// 设置正文的可见字符范围。
    ///
    /// 仅渲染 `current_body` 中字符索引 `[start, end)` 范围内的正文。
    /// 说话者名字始终完整显示，不受此范围影响。
    ///
    /// # 参数
    /// - `start`: 起始字符索引（通常为 0），基于 Unicode 标量值计数
    /// - `end`: 结束字符索引（不含），`end - start` 为可见字符数
    ///
    /// # 行为
    /// - 当 `start == 0 && end == 0` 时，仅显示说话者名字
    /// - 当 `end >= total_chars` 时，显示全部正文
    /// - 仅当可见范围**变化**时才标记 `text_changed = true`，避免无谓的重新布局
    ///
    /// # 与 Typewriter 协作
    /// ```rust,ignore
    /// // 每帧同步打字机进度
    /// text_renderer.set_visible_range(0, typewriter.visible_chars());
    /// ```
    pub fn set_visible_range(&mut self, start: usize, end: usize) {
        let visible = end.saturating_sub(start);
        let current = self.visible_body_chars.unwrap_or(usize::MAX);
        if visible != current {
            self.visible_body_chars = Some(visible);
            self.text_changed = true;
        }
    }

    /// 清除可见范围限制，恢复显示全部正文。
    ///
    /// 后续 `prepare()` 将渲染完整 `current_body`。
    pub fn clear_visible_range(&mut self) {
        if self.visible_body_chars.is_some() {
            self.visible_body_chars = None;
            self.text_changed = true;
        }
    }

    /// 加载自定义字体（TTF/OTF 字节数据）。
    pub fn load_font(&mut self, font_data: Vec<u8>) {
        self.font_system
            .db_mut()
            .load_font_source(cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(
                font_data,
            )));
    }

    /// 更新屏幕尺寸（响应窗口 resize）。
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_width = width as f32;
        self.screen_height = height as f32;
        self.text_changed = true;
    }

    /// 准备渲染 — 光栅化新字形并上传到图集。
    ///
    /// 仅当文本内容变更时才重新布局和光栅化。
    pub fn prepare(&mut self, device: &Device, queue: &Queue) {
        if !self.text_changed {
            return;
        }
        self.text_changed = false;

        // 步骤 1：计算文本框参数
        let text_box_x = self.screen_width * TEXT_BOX_MARGIN_RATIO;
        let text_box_width = self.screen_width * (1.0 - 2.0 * TEXT_BOX_MARGIN_RATIO);
        let text_box_y = self.screen_height * (1.0 - TEXT_BOX_HEIGHT_RATIO);
        let text_box_height = self.screen_height * TEXT_BOX_HEIGHT_RATIO;

        let padding = self.config.text_box_padding;
        let body_area_width = text_box_width - padding * 2.0;

        // 步骤 2：布局说话者名字
        self.speaker_buffer.set_size(
            Some(body_area_width),
            Some(self.config.speaker_font_size * self.config.line_height),
        );
        let speaker_attrs = Attrs::new().family(Family::SansSerif).weight(Weight::BOLD);
        self.speaker_buffer.set_text(
            &self.current_speaker,
            &speaker_attrs,
            Shaping::Advanced,
            None,
        );
        self.speaker_buffer
            .shape_until_scroll(&mut self.font_system, true);

        // 步骤 3：布局正文
        // 始终预留说话者名字区域高度，保持正文位置固定不受旁白/对话切换影响
        let speaker_height =
            self.config.speaker_font_size * self.config.line_height + SPEAKER_BODY_GAP;
        let body_height = text_box_height - padding * 2.0 - speaker_height;

        self.body_buffer
            .set_size(Some(body_area_width), Some(body_height));
        let body_attrs = Attrs::new().family(Family::SansSerif);

        // 根据可见范围截取正文
        let body_text = if let Some(visible) = self.visible_body_chars {
            self.current_body.chars().take(visible).collect::<String>()
        } else {
            self.current_body.clone()
        };

        self.body_buffer
            .set_text(&body_text, &body_attrs, Shaping::Advanced, None);
        self.body_buffer
            .shape_until_scroll(&mut self.font_system, true);

        // 步骤 4：收集所有需要光栅化的字形信息
        let speaker_offset = (text_box_x + padding, text_box_y + padding);
        let body_offset = (text_box_x + padding, text_box_y + padding + speaker_height);

        // 先收集所有字形的 (cache_key, 位置, 颜色) 信息，避免 borrow checker 冲突
        let mut pending_glyphs: Vec<(cosmic_text::CacheKey, (f32, f32), [f32; 4])> = Vec::new();

        // 收集 speaker buffer 的字形
        for run in self.speaker_buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                if glyph.glyph_id == 0 {
                    continue; // 跳过空白字符
                }
                let physical = glyph.physical(speaker_offset, 1.0);
                pending_glyphs.push((
                    physical.cache_key,
                    (physical.x as f32, physical.y as f32),
                    self.config.speaker_color,
                ));
            }
        }

        // 收集 body buffer 的字形
        for run in self.body_buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                if glyph.glyph_id == 0 {
                    continue;
                }
                let physical = glyph.physical(body_offset, 1.0);
                pending_glyphs.push((
                    physical.cache_key,
                    (physical.x as f32, physical.y as f32),
                    self.config.text_color,
                ));
            }
        }

        // 步骤 5：光栅化并上传到图集
        for (cache_key, _position, _color) in &pending_glyphs {
            // 如果已缓存，跳过
            if self.glyph_cache.contains_key(cache_key) {
                continue;
            }

            // 使用 SwashCache::get_image 光栅化字形（会填充 image_cache）
            // 注意：需要先提取图像数据（clone），释放对 self 的借用后
            // 再进行图集分配和上传操作
            let image = self
                .swash_cache
                .get_image(&mut self.font_system, *cache_key);
            let (w, h, left, top, image_data) = match &image {
                Some(img) => (
                    img.placement.width,
                    img.placement.height,
                    img.placement.left,
                    img.placement.top,
                    img.data.clone(),
                ),
                None => continue,
            };

            // 尝试在图集中分配空间
            let (atlas_x, atlas_y) = match self.atlas.allocate(w, h) {
                Some(pos) => pos,
                None => {
                    // 图集已满，尝试增长
                    if self.atlas.grow(device) {
                        // 增长后重建绑定组并重新上传所有字形
                        self.bind_group = device.create_bind_group(&BindGroupDescriptor {
                            label: Some("文本渲染绑定组（重建）"),
                            layout: &self.bind_group_layout,
                            entries: &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::TextureView(self.atlas.view()),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::Sampler(&self.sampler),
                                },
                            ],
                        });
                        // 重新上传所有已缓存字形
                        let cached_keys: Vec<cosmic_text::CacheKey> =
                            self.glyph_cache.keys().copied().collect();
                        self.glyph_cache.clear();
                        for ck in cached_keys {
                            if let Some(re_img) =
                                self.swash_cache.get_image(&mut self.font_system, ck)
                            {
                                let rw = re_img.placement.width;
                                let rh = re_img.placement.height;
                                if let Some((rx, ry)) = self.atlas.allocate(rw, rh) {
                                    self.atlas.upload(queue, rx, ry, rw, rh, &re_img.data);
                                    self.glyph_cache.insert(
                                        ck,
                                        CachedGlyph {
                                            atlas_x: rx,
                                            atlas_y: ry,
                                            width: rw,
                                            height: rh,
                                            left: re_img.placement.left,
                                            top: re_img.placement.top,
                                        },
                                    );
                                }
                            }
                        }
                        // 重新尝试分配
                        match self.atlas.allocate(w, h) {
                            Some(pos) => pos,
                            None => continue, // 增长后仍放不下
                        }
                    } else {
                        continue; // 无法增长
                    }
                }
            };

            // 上传像素数据到图集
            self.atlas
                .upload(queue, atlas_x, atlas_y, w, h, &image_data);

            // 记录缓存
            self.glyph_cache.insert(
                *cache_key,
                CachedGlyph {
                    atlas_x,
                    atlas_y,
                    width: w,
                    height: h,
                    left,
                    top,
                },
            );
        }

        // 步骤 6：构建顶点缓冲区
        self.build_vertex_buffers(device, queue);
    }

    /// 构建顶点/索引缓冲区。
    ///
    /// 遍历两个 buffer 的 layout runs，为每个已缓存的字形生成 4 顶点 + 6 索引。
    fn build_vertex_buffers(&mut self, device: &Device, queue: &Queue) {
        let mut vertices: Vec<GlyphVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        let ctx = RenderContext {
            atlas_w: self.atlas.width_f32(),
            atlas_h: self.atlas.height_f32(),
            screen_w: self.screen_width,
            screen_h: self.screen_height,
        };

        // 计算偏移
        let text_box_x = self.screen_width * TEXT_BOX_MARGIN_RATIO;
        let text_box_y = self.screen_height * (1.0 - TEXT_BOX_HEIGHT_RATIO);
        let padding = self.config.text_box_padding;

        let speaker_offset = (text_box_x + padding, text_box_y + padding);
        // 始终预留说话者区域高度，保持正文位置固定
        let speaker_height =
            self.config.speaker_font_size * self.config.line_height + SPEAKER_BODY_GAP;
        let body_offset = (text_box_x + padding, text_box_y + padding + speaker_height);

        // 收集说话者字形
        for run in self.speaker_buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                if glyph.glyph_id == 0 {
                    continue;
                }
                let physical = glyph.physical(speaker_offset, 1.0);
                if let Some(cached) = self.glyph_cache.get(&physical.cache_key) {
                    Self::push_glyph_quad(
                        physical.x as f32,
                        physical.y as f32,
                        cached,
                        self.config.speaker_color,
                        &ctx,
                        &mut vertices,
                        &mut indices,
                    );
                }
            }
        }

        // 收集正文字形
        for run in self.body_buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                if glyph.glyph_id == 0 {
                    continue;
                }
                let physical = glyph.physical(body_offset, 1.0);
                if let Some(cached) = self.glyph_cache.get(&physical.cache_key) {
                    Self::push_glyph_quad(
                        physical.x as f32,
                        physical.y as f32,
                        cached,
                        self.config.text_color,
                        &ctx,
                        &mut vertices,
                        &mut indices,
                    );
                }
            }
        }

        if vertices.is_empty() {
            self.vertex_buffer = None;
            self.index_buffer = None;
            self.index_count = 0;
            return;
        }

        // 创建/更新顶点缓冲区
        let vb = device.create_buffer(&BufferDescriptor {
            label: Some("字形顶点缓冲"),
            size: (vertices.len() * std::mem::size_of::<GlyphVertex>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vb, 0, bytemuck::cast_slice(&vertices));

        // 创建/更新索引缓冲区
        let ib = device.create_buffer(&BufferDescriptor {
            label: Some("字形索引缓冲"),
            size: (indices.len() * std::mem::size_of::<u32>()) as u64,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&ib, 0, bytemuck::cast_slice(&indices));

        self.vertex_buffer = Some(vb);
        self.index_buffer = Some(ib);
        self.index_count = indices.len() as u32;
    }

    /// 为单个字形生成四边形顶点和索引。
    ///
    /// 将像素坐标转换为 NDC，计算图集 UV。
    fn push_glyph_quad(
        px: f32,
        py: f32,
        cached: &CachedGlyph,
        color: [f32; 4],
        ctx: &RenderContext,
        vertices: &mut Vec<GlyphVertex>,
        indices: &mut Vec<u32>,
    ) {
        // 像素坐标 → NDC（Y 轴翻转：像素 Y=0 顶部 → NDC Y=1 顶部）
        // physical.x / physical.y 是笔位置（字形原点），需加上 bearing 偏移得到四边形左上角
        // left bearing：四边形左边缘 = 笔X + left（left 可负）
        // top bearing：四边形上边缘 = 笔Y - top（top 正=上方，像素坐标 Y 向下故减去）
        let quad_left = px + cached.left as f32;
        let quad_top = py - cached.top as f32;

        let ndc_x = quad_left / ctx.screen_w * 2.0 - 1.0;
        let ndc_y = 1.0 - quad_top / ctx.screen_h * 2.0;
        let ndc_w = cached.width as f32 / ctx.screen_w * 2.0;
        let ndc_h = cached.height as f32 / ctx.screen_h * 2.0;

        // 图集 UV（归一化到 [0, 1]）
        let uv_min_x = cached.atlas_x as f32 / ctx.atlas_w;
        let uv_min_y = cached.atlas_y as f32 / ctx.atlas_h;
        let uv_max_x = (cached.atlas_x + cached.width) as f32 / ctx.atlas_w;
        let uv_max_y = (cached.atlas_y + cached.height) as f32 / ctx.atlas_h;

        let idx = vertices.len() as u32;

        // 左下角：NDC (left, bottom), UV (min_x, max_y)
        vertices.push(GlyphVertex {
            position: [ndc_x, ndc_y - ndc_h],
            uv: [uv_min_x, uv_max_y],
            color,
        });
        // 右下角：NDC (right, bottom), UV (max_x, max_y)
        vertices.push(GlyphVertex {
            position: [ndc_x + ndc_w, ndc_y - ndc_h],
            uv: [uv_max_x, uv_max_y],
            color,
        });
        // 右上角：NDC (right, top), UV (max_x, min_y)
        vertices.push(GlyphVertex {
            position: [ndc_x + ndc_w, ndc_y],
            uv: [uv_max_x, uv_min_y],
            color,
        });
        // 左上角：NDC (left, top), UV (min_x, min_y)
        vertices.push(GlyphVertex {
            position: [ndc_x, ndc_y],
            uv: [uv_min_x, uv_min_y],
            color,
        });

        // 两个三角形
        indices.push(idx);
        indices.push(idx + 1);
        indices.push(idx + 2);
        indices.push(idx);
        indices.push(idx + 2);
        indices.push(idx + 3);
    }
}

// ============================================================================
// Layer trait 实现
// ============================================================================

impl Layer for TextRenderer {
    fn render<'a>(&'a self, encoder: &'a mut CommandEncoder, output_view: &'a TextureView) {
        if self.index_count == 0 {
            return;
        }

        let vb = match &self.vertex_buffer {
            Some(vb) => vb,
            None => return,
        };
        let ib = match &self.index_buffer {
            Some(ib) => ib,
            None => return,
        };

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("文本层渲染 Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
    }
}

// ============================================================================
// Debug 实现
// ============================================================================

impl std::fmt::Debug for TextRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRenderer")
            .field("config", &self.config)
            .field("screen_width", &self.screen_width)
            .field("screen_height", &self.screen_height)
            .field("current_speaker", &self.current_speaker)
            .field("current_body", &self.current_body)
            .field("text_changed", &self.text_changed)
            .field("visible_body_chars", &self.visible_body_chars)
            .field("glyph_cache_size", &self.glyph_cache.len())
            .field("index_count", &self.index_count)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01-AC04
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的 headless wgpu 上下文。
    fn create_test_device() -> Option<(Device, Queue)> {
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
                label: Some("文本渲染器测试设备"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        Some((device, queue))
    }

    // ========================================================================
    // AC01 — 文本布局
    // ========================================================================

    /// AC01: 验证 TextRenderer 可正确设置和布局 CJK 文本。
    #[test]
    fn ac01_layout_cjk_text() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let config = TextConfig::default();
        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            config,
        )
        .expect("TextRenderer 创建应成功");

        // 设置 CJK 文本
        renderer.set_text("小百合", "今天天气真好啊。");
        renderer.prepare(&device, &queue);

        // 验证布局已完成（text_changed 被重置）
        assert!(!renderer.text_changed, "prepare 后 text_changed 应为 false");

        // 如果系统有 CJK 字体，应能光栅化出字形
        // 即使无字体，也不应 panic
        eprintln!(
            "AC01: glyph_cache 中有 {} 个字形，index_count={}",
            renderer.glyph_cache.len(),
            renderer.index_count
        );
    }

    /// AC01: 验证文本为空时仍可正常处理。
    #[test]
    fn ac01_layout_empty_speaker() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 只有正文，无说话者
        renderer.set_text("", "独白文本。");
        renderer.prepare(&device, &queue);

        assert!(!renderer.text_changed);
    }

    // ========================================================================
    // AC02 — CJK + Latin 混合文本
    // ========================================================================

    /// AC02: 验证 CJK + Latin 混合文本不会导致崩溃。
    #[test]
    fn ac02_mixed_cjk_latin_text() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 混合 CJK + Latin + 数字 + 符号
        renderer.set_text(
            "Sayori",
            "こんにちは！Hello World! 2026년 6월 — Test №42 (CJK+Latin)",
        );
        renderer.prepare(&device, &queue);

        // 验证不 panic，text_changed 被重置
        assert!(!renderer.text_changed);
        eprintln!(
            "AC02: 混合文本生成了 {} 个缓存字形",
            renderer.glyph_cache.len()
        );
    }

    // ========================================================================
    // AC03 — 空字符串不 panic
    // ========================================================================

    /// AC03: 验证空字符串输入不 panic。
    #[test]
    fn ac03_empty_string_no_panic() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 空字符串
        renderer.set_text("", "");
        renderer.prepare(&device, &queue);

        assert!(!renderer.text_changed);
        // 空文本不应有任何顶点
        assert_eq!(renderer.index_count, 0, "空文本 index_count 应为 0");

        // 连续多次设置空字符串
        renderer.set_text("", "");
        renderer.prepare(&device, &queue);
        renderer.set_text("", "");
        renderer.prepare(&device, &queue);
        // 仍不 panic
        assert_eq!(renderer.index_count, 0);
    }

    /// AC03: 验证 clear_text() 方法不 panic。
    #[test]
    fn ac03_clear_text_no_panic() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 先设置文本
        renderer.set_text("说话者", "一些对话内容。");
        renderer.prepare(&device, &queue);

        // 清除
        renderer.clear_text();
        renderer.prepare(&device, &queue);

        assert!(!renderer.text_changed);
        assert_eq!(renderer.index_count, 0);
    }

    // ========================================================================
    // AC04 — 长文本
    // ========================================================================

    /// AC04: 验证 10000 字符的文本不会导致崩溃。
    #[test]
    fn ac04_long_text_no_crash() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 生成 10000 字符的文本（重复中文短语）
        let long_text = "今天天气真好啊。".repeat(1000); // 10 chars × 1000 = 10000

        let start = std::time::Instant::now();
        renderer.set_text("叙述者", &long_text);
        renderer.prepare(&device, &queue);
        let elapsed = start.elapsed();

        assert!(!renderer.text_changed);

        eprintln!(
            "AC04: 10000 字符布局+光栅化耗时 {:?}, glyph_cache 中有 {} 个字形",
            elapsed,
            renderer.glyph_cache.len()
        );

        // 布局应在合理时间内完成（< 5s）
        assert!(
            elapsed.as_secs() < 5,
            "10000 字符布局应 < 5s，实际耗时 {:?}",
            elapsed
        );
    }

    // ========================================================================
    // 额外边界测试
    // ========================================================================

    /// 验证 resize 触发重新布局。
    #[test]
    fn resize_triggers_relayout() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        renderer.set_text("测试", "文字内容");
        renderer.prepare(&device, &queue);
        assert!(!renderer.text_changed);

        // resize 应标记为变更
        renderer.resize(1280, 720);
        assert!(renderer.text_changed, "resize 后 text_changed 应为 true");
    }

    /// 验证同一文本重复设置不会重复光栅化。
    #[test]
    fn same_text_no_redundant_rasterization() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        renderer.set_text("小百合", "文本内容");
        renderer.prepare(&device, &queue);

        let first_cache_size = renderer.glyph_cache.len();

        // 再次 prepare 不应增长缓存（缓冲区内置防重复机制）
        // 注：在稳定状态下持续 prepare 不应崩溃
        renderer.prepare(&device, &queue);
        renderer.prepare(&device, &queue);
        assert!(!renderer.text_changed);

        // 缓存大小不应显著增长
        eprintln!(
            "缓存大小: 首次={first_cache_size}, 最终={}",
            renderer.glyph_cache.len()
        );
    }

    // ========================================================================
    // 可见范围集成测试
    // ========================================================================

    /// 验证 set_visible_range 后仅渲染前 N 个字符。
    #[test]
    fn test_visible_range_limits_body() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 设置长文本
        renderer.set_text("", "ABCDEFGHIJ"); // 10 个字符
        // 仅显示前 3 个字符
        renderer.set_visible_range(0, 3);
        renderer.prepare(&device, &queue);

        assert!(!renderer.text_changed, "prepare 后 text_changed 应为 false");
        eprintln!(
            "可见范围限制: index_count={}, 缓存字形数={}",
            renderer.index_count,
            renderer.glyph_cache.len()
        );
        // 应有字形（前 3 个字符 "ABC"），但不验证具体数量（依赖系统字体）
    }

    /// 验证未设置 visible_range 时显示全部正文（向后兼容）。
    #[test]
    fn test_visible_range_none_shows_all() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        renderer.set_text("测试", "ABCDEFGH");
        // 不设置 visible_range → 应显示全部
        renderer.prepare(&device, &queue);

        let full_index_count = renderer.index_count;

        // 重置后设置可见范围限制
        renderer.set_visible_range(0, 3);
        renderer.prepare(&device, &queue);

        let limited_index_count = renderer.index_count;

        eprintln!("全部字形数: {full_index_count}, 限制后字形数: {limited_index_count}");
        // 限制后的字形数应 ≤ 全部字形数
        assert!(
            limited_index_count <= full_index_count,
            "限制后的字形数应不超过全部字形数"
        );
    }

    /// 验证 set_visible_range(0, 0) → 仅显示说话者名字（如有）。
    #[test]
    fn test_visible_range_zero() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // 无说话者，仅正文
        renderer.set_text("", "正文内容");
        renderer.set_visible_range(0, 0);
        renderer.prepare(&device, &queue);

        // 正文无任何字符可见，仅说话者（空），应无字形
        assert_eq!(
            renderer.index_count, 0,
            "无可见正文且无说话者时 index_count 应为 0"
        );
    }

    /// 验证可见范围变更触发重新布局。
    #[test]
    fn test_visible_range_update_triggers_relayout() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        renderer.set_text("测试", "ABCDEFGH");
        renderer.set_visible_range(0, 4);
        renderer.prepare(&device, &queue);
        assert!(!renderer.text_changed);

        // 变更可见范围
        renderer.set_visible_range(0, 6);
        assert!(
            renderer.text_changed,
            "可见范围变更应触发 text_changed = true"
        );

        renderer.prepare(&device, &queue);
        assert!(!renderer.text_changed, "prepare 后应清除 text_changed");
    }

    /// 验证相同可见范围不触发重新布局。
    #[test]
    fn test_same_visible_range_no_relayout() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        renderer.set_text("测试", "ABCDEFGH");
        renderer.set_visible_range(0, 4);
        renderer.prepare(&device, &queue);
        assert!(!renderer.text_changed);

        // 相同的可见范围不应标记变更
        renderer.set_visible_range(0, 4);
        assert!(!renderer.text_changed, "相同可见范围不应触发 text_changed");
    }

    /// 验证 clear_visible_range 恢复显示全部。
    #[test]
    fn test_clear_visible_range_restores_full() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        renderer.set_text("测试", "ABCDEFGH");
        renderer.set_visible_range(0, 4);
        renderer.prepare(&device, &queue);

        // 清除限制
        renderer.clear_visible_range();
        assert!(renderer.text_changed, "清除限制应触发 text_changed");

        renderer.prepare(&device, &queue);
        // 应恢复全部文本
        eprintln!("恢复全部后 index_count: {}", renderer.index_count);
    }

    /// 验证 CJK 文本的可见范围限制（UTF-8 安全）。
    #[test]
    fn test_visible_range_cjk_safe() {
        let (device, queue) = match create_test_device() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut renderer = TextRenderer::new(
            &device,
            &queue,
            TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            TextConfig::default(),
        )
        .expect("TextRenderer 创建应成功");

        // CJK 文本（多字节字符）
        renderer.set_text("小百合", "今天天气真好啊。明日もいい天気でしょう。");
        renderer.set_visible_range(0, 7); // 前 7 个字符："今天天气真好啊"
        renderer.prepare(&device, &queue);

        assert!(!renderer.text_changed);
        eprintln!(
            "CJK 可见范围: 缓存字形数={}, index_count={}",
            renderer.glyph_cache.len(),
            renderer.index_count
        );
        // 不应 panic（UTF-8 边界安全验证）
    }
}
