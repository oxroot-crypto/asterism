// Asterism — Galgame/ADV 游戏引擎
//
// 文件路径：engine/aster-renderer/src/shaders/text.wgsl
// 功能概述：文本着色器 — 将字形图集中的 Alpha 蒙版字形渲染为彩色文本。
//           顶点着色器接收 NDC 空间的四边形顶点 + UV + 颜色；
//           片元着色器采样 R8Unorm 字形图集，以字形 Alpha 为蒙版输出文本颜色。
// 作者：Claude (AI)
// 创建日期：2026-06-14
//
// 绑定组：
//   @group(0): 字形图集纹理 + 采样器
//     @binding(0): R8Unorm 2D 纹理（字形 Alpha 蒙版图集）
//     @binding(1): 过滤采样器（线性过滤，ClampToEdge 寻址）
//
// Alpha 混合：由 Rust 端管线配置 BlendState::ALPHA_BLENDING
//   src = SrcAlpha, dst = OneMinusSrcAlpha
//   输出: text_color * glyph_alpha + background * (1 - glyph_alpha)

// ============================================================================
// 结构体定义
// ============================================================================

/// 顶点着色器输出 — 传递 UV 坐标和颜色到片元着色器。
///
/// `@builtin(position)` 由光栅化器用于像素插值，
/// `@location(0)` 的 UV 坐标用于采样字形图集，
/// `@location(1)` 的颜色用于为字形蒙版着色。
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

// ============================================================================
// 顶点输入结构体（对应 Rust 端的 GlyphVertex）
// ============================================================================

/// 字形顶点 — 对应 Rust 端的 `GlyphVertex` 结构体（通过 bytemuck 派生 Pod/Zeroable）。
///
/// 内存布局（32 字节）：
///   - position: vec2<f32> — NDC 空间四边形顶点位置（偏移 0，8 字节）
///   - uv: vec2<f32> — 字形图集 UV 坐标（偏移 8，8 字节）
///   - color: vec4<f32> — RGBA 文本颜色（偏移 16，16 字节）
///
/// 每个字形由 4 个顶点 + 6 个索引组成一个四边形。
/// 所有字形合并到同一个顶点/索引缓冲区中，通过一次 draw call 渲染。
struct GlyphVertex {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

// ============================================================================
// 绑定组
// ============================================================================

/// @group(0): 字形图集绑定组
///   @binding(0): R8Unorm 2D 纹理 — 字形 Alpha 蒙版图集
///   @binding(1): 采样器 — 线性过滤，ClampToEdge 寻址
///
/// 每个字形在渲染前被光栅化并上传到图集，
/// 着色器通过 UV 坐标采样图集获取字形轮廓。
@group(0) @binding(0) var t_glyph_atlas: texture_2d<f32>;
@group(0) @binding(1) var s_glyph: sampler;

// ============================================================================
// 顶点着色器
// ============================================================================

/// 文本顶点着色器 — 将 NDC 空间的字形顶点直接输出。
///
/// # 输入
/// - `@location(0) position: vec2<f32>` — NDC 空间的顶点位置（已由 CPU 端计算）
/// - `@location(1) uv: vec2<f32>` — 字形图集 UV 坐标
/// - `@location(2) color: vec4<f32>` — RGBA 文本颜色
///
/// # 坐标约定
/// - CPU 端已将像素坐标转换为 NDC（x: -1 左 → 1 右，y: -1 底 → 1 顶）
/// - z=0, w=1 用于正交投影
/// - 图集 UV: (0,0) 左上角 → (1,1) 右下角
///
/// # 实现说明
/// 位置和 UV 均在 CPU 端（Rust）完成计算后传入。
/// 着色器仅做透传，将输入直接映射到输出。
/// 这种设计简化了着色器逻辑，将变换计算集中到 Rust 端。
@vertex
fn vs_main(in: GlyphVertex) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

// ============================================================================
// 片元着色器
// ============================================================================

/// 文本片元着色器 — 采样字形图集 Alpha 蒙版，应用文本颜色。
///
/// # 输出
/// `@location(0) vec4<f32>` — 预乘 Alpha 的 RGBA 颜色。
///
/// # 渲染流程
/// 1. 从图集采样字形 Alpha 值（R8Unorm → f32，0.0=背景，1.0=字形）
/// 2. 以 Alpha 为蒙版：输出颜色 = 文本颜色 × Alpha
/// 3. 配合 Alpha 混合管线：`src=SrcAlpha, dst=OneMinusSrcAlpha`
///    最终像素 = text_color * alpha + background * (1 - alpha)
///
/// # 抗锯齿说明
/// swash 光栅化器生成灰度抗锯齿字形（Alpha 通道含部分透明像素），
/// 线性采样器在缩放时提供额外平滑。
/// 不使用 MSDF 或有符号距离场——字形图集直接包含光栅化位图。
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 步骤 1：采样字形图集，获取 Alpha 蒙版值
    // R8Unorm 纹理中每个像素存储字形的覆盖率（0.0 = 完全透明，1.0 = 完全不透明）
    let alpha = textureSample(t_glyph_atlas, s_glyph, in.uv).r;

    // 步骤 2：以 Alpha 为蒙版输出预乘颜色
    // 预乘 Alpha 格式配合 BlendState::ALPHA_BLENDING
    return vec4<f32>(in.color.rgb * alpha, in.color.a * alpha);
}
