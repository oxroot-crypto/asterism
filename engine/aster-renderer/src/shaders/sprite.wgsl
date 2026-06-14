// Asterism — Galgame/ADV 游戏引擎
//
// 文件路径：engine/aster-renderer/src/shaders/sprite.wgsl
// 功能概述：立绘精灵着色器 — 将带 Alpha 通道的角色立绘纹理以四边形方式渲染到屏幕指定位置。
//           顶点着色器接受单位四边形顶点，通过 uniform 中的位置和缩放参数变换到 NDC 空间；
//           片元着色器采样纹理并应用 alpha 透明度混合，实现立绘与背景的叠加显示。
// 作者：Claude (AI)
// 创建日期：2026-06-14
//
// 参考：基于 BackgroundLayer 着色器模式，新增 alpha 混合和逐实例 uniform 支持。

// ============================================================================
// 结构体定义
// ============================================================================

/// 顶点着色器输出 — 传递 UV 坐标到片元着色器进行纹理采样。
///
/// `@builtin(position)` 由光栅化器用于像素插值，
/// `@location(0)` 的 UV 坐标传递给片元着色器。
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

/// 立绘精灵 Uniform — 控制单个立绘在屏幕上的位置、大小和透明度。
///
/// 对应 Rust 端的 `SpriteUniform` 结构体（通过 bytemuck 派生 Pod/Zeroable）。
/// 内存布局（24 字节）：
///   - sprite_pos: vec2<f32> — 立绘中心在 NDC 空间的位置（偏移 0，8 字节）
///   - sprite_half_size: vec2<f32> — 立绘四边形在 NDC 空间的半尺寸（偏移 8，8 字节）
///   - alpha: f32 — 透明度（0.0=全透明，1.0=不透明）（偏移 16，4 字节）
///   - _padding: f32 — WGSL struct 对齐填充（偏移 20，4 字节）
struct SpriteUniform {
    sprite_pos: vec2<f32>,
    sprite_half_size: vec2<f32>,
    alpha: f32,
    // WGSL struct 自动对齐到最大成员对齐的倍数（vec2 = 8 字节对齐）
    // 20 字节 → 填充至 24 字节
}

// ============================================================================
// 绑定组
// ============================================================================

/// @group(0): 纹理绑定组 — 立绘纹理 + 采样器
///   @binding(0): 2D 纹理（rgba8unorm-srgb，含 Alpha 通道）
///   @binding(1): 纹理采样器（线性过滤，ClampToEdge 寻址）
///
/// 此布局与 `Texture::bind_group` 完全一致，
/// 可直接复用 Texture 已创建的绑定组。
@group(0) @binding(0) var t_sprite: texture_2d<f32>;
@group(0) @binding(1) var s_sprite: sampler;

/// @group(1): 逐立绘 Uniform 绑定组 — 每个立绘独立的变换参数
///   @binding(0): SpriteUniform（位置 + 半尺寸 + alpha）
@group(1) @binding(0) var<uniform> u_sprite: SpriteUniform;

// ============================================================================
// 顶点着色器
// ============================================================================

/// 立绘精灵顶点着色器 — 将单位四边形变换到 NDC 空间的指定位置和尺寸。
///
/// # 输入
/// - `@location(0) position: vec2<f32>` — 单位四边形的顶点坐标（范围 (-0.5, -0.5) ~ (0.5, 0.5)）
/// - `@location(1) uv: vec2<f32>` — 纹理坐标（范围 (0, 0) ~ (1, 1)）
///
/// # 变换流程
/// 1. 将单位四边形顶点乘以 `sprite_half_size` 得到半尺寸四边形
/// 2. 平移到 `sprite_pos` 指定的 NDC 位置
/// 3. 输出 NDC 坐标（z=0, w=1）和 UV 坐标
///
/// # 坐标约定
/// - 单位四边形中心在原点 (0, 0)，范围 (-0.5, -0.5) ~ (0.5, 0.5)
/// - NDC: (-1, -1) = 左下角, (1, 1) = 右上角
/// - wgpu 纹理坐标: (0, 0) = 左上角, (1, 1) = 右下角
/// - UV 输入已预翻转 V 轴（见顶点缓冲定义）
@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
) -> VertexOutput {
    var output: VertexOutput;

    // 步骤 1：缩放 — 将单位四边形顶点乘以半尺寸，得到 NDC 空间的半尺寸四边形
    // 步骤 2：平移 — 加上 sprite_pos 将四边形移动到目标位置
    let ndc_x = position.x * u_sprite.sprite_half_size.x * 2.0 + u_sprite.sprite_pos.x;
    let ndc_y = position.y * u_sprite.sprite_half_size.y * 2.0 + u_sprite.sprite_pos.y;

    output.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    output.uv = uv;
    return output;
}

// ============================================================================
// 片元着色器
// ============================================================================

/// 立绘精灵片元着色器 — 采样纹理并应用 alpha 透明度。
///
/// # 输出
/// `@location(0) vec4<f32>` — RGBA 颜色，alpha 通道由纹理 alpha × uniform alpha 计算。
///
/// # Alpha 混合
/// 使用 wgpu `BlendState::ALPHA_BLENDING` 进行预乘 Alpha 混合：
/// - src = SrcAlpha（纹理自身的 alpha × uniform alpha）
/// - dst = OneMinusSrcAlpha
/// - 结果：`output = texture_color * alpha + background * (1 - alpha)`
///
/// # 透明度应用
/// 纹理的 RGB 通道保持不变（非预乘），仅将 uniform alpha 乘到片段的 alpha 通道。
/// 这对于透明立绘（如淡入淡出效果）非常重要——降低 alpha 使立绘半透明，
/// 露出下方图层。
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // 步骤 1：采样纹理
    let color = textureSample(t_sprite, s_sprite, input.uv);

    // 步骤 2：应用 uniform alpha 到纹理的 alpha 通道
    // 纹理 RGB 不变，A 乘以 uniform alpha
    // 这允许动态调整立绘透明度（如 fade 效果）
    let final_alpha = color.a * u_sprite.alpha;

    // 步骤 3：返回预乘结果
    // 注意：这里使用预乘 alpha（premultiplied alpha），
    // 配合 BlendState 的 SrcAlpha / OneMinusSrcAlpha 模式
    return vec4<f32>(color.rgb * final_alpha, final_alpha);
}
