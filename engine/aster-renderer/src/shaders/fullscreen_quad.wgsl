// Asterism — Galgame/ADV 游戏引擎
//
// 文件路径：engine/aster-renderer/src/shaders/fullscreen_quad.wgsl
// 功能概述：全屏四边形着色器 — 使用无顶点缓冲的大三角形技术渲染全屏背景纹理。
//           顶点着色器通过 @builtin(vertex_index) 生成覆盖整个裁剪空间的三角形，
//           片元着色器采样纹理并根据 uniform 参数进行宽高比适配（cover 裁剪模式）。
// 作者：Claude (AI)
// 创建日期：2026-06-14
//
// 参考：基于 "Hello Triangle" 全屏四边形变体（wgpu 示例常用模式）

// ============================================================================
// 结构体定义
// ============================================================================

/// 顶点着色器输出 — 传递裁剪空间位置和 UV 坐标到片元着色器
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

/// 适配 Uniform — 控制纹理在窗口中的缩放与对齐方式
///
/// 对应 Rust 端的 `FitUniform` 结构体（通过 bytemuck 派生 Pod/Zeroable）。
/// 内存布局必须与 Rust 端一致（16 字节对齐的 vec2 × 2 + 1 个 f32 → 填充到 32 字节）。
struct FitUniform {
    texture_size: vec2<f32>,   // 纹理像素尺寸 (width, height)
    window_size: vec2<f32>,    // 窗口逻辑像素尺寸 (width, height)
    fit_mode: f32,              // 适配模式：0.0 = contain（留黑边），1.0 = cover（裁剪填充）
    // WGSL struct 自动对齐：vec2×2 = 16 bytes + f32 = 4 bytes → 20 bytes，填充到 32 bytes
}

// ============================================================================
// 绑定组
// ============================================================================

/// @group(0): 纹理绑定组 — 当前背景纹理 + 采样器
///   @binding(0): 2D 纹理（rgba8unorm-srgb）
///   @binding(1): 纹理采样器
@group(0) @binding(0) var t_background: texture_2d<f32>;
@group(0) @binding(1) var s_background: sampler;

/// @group(1): 适配 Uniform 绑定组 — 纹理尺寸 + 窗口尺寸 + 适配模式
///   @binding(0): FitUniform
@group(1) @binding(0) var<uniform> u_fit: FitUniform;

// ============================================================================
// 顶点着色器
// ============================================================================

/// 全屏三角形顶点着色器 — 无顶点缓冲方案。
///
/// 通过 `@builtin(vertex_index)` 从 3 个顶点生成覆盖整个 NDC 裁剪空间的大三角形。
/// 三角形顶点：
///   - vertex_index=0: NDC (-1, -1, 0, 1) → UV (0, 1)  左下角
///   - vertex_index=1: NDC ( 3, -1, 0, 1) → UV (2, 1)  右下角（超出屏幕右侧）
///   - vertex_index=2: NDC (-1,  3, 0, 1) → UV (0, -1) 左上角（超出屏幕顶部）
///
/// 大三角形覆盖整个屏幕，GPU 的 clip 机制自动裁剪超出部分，
/// 等价于两个三角形组成的全屏四边形，但只需 3 个顶点和零缓冲。
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // 根据 vertex_index 查表获取 NDC 坐标和 UV
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),  // vertex 0: 左下
        vec2<f32>( 3.0, -1.0),  // vertex 1: 右下
        vec2<f32>(-1.0,  3.0),  // vertex 2: 左上
    );

    // wgpu 纹理坐标：(0,0)=左上角，(1,1)=右下角
    // NDC: (-1,-1)=屏幕左下角，(1,1)=屏幕右上角
    // 因此需要翻转 V 坐标：屏幕底部对应纹理底部(v=1)，屏幕顶部对应纹理顶部(v=0)
    let uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),  // vertex 0: 屏幕左下 → 纹理左下
        vec2<f32>(2.0, 1.0),  // vertex 1: 屏幕右下 → 纹理右下
        vec2<f32>(0.0, -1.0), // vertex 2: 屏幕左上 → 纹理左上
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

// ============================================================================
// 片元着色器
// ============================================================================

/// 全屏四边形片元着色器 — 采样背景纹理并执行宽高比适配。
///
/// # 宽高比适配算法
///
/// **Cover 模式**（`fit_mode == 1.0`，默认）：
///   缩放纹理以完全覆盖窗口，超出部分裁剪，无黑边。
///   算法：计算 `scale = max(window_aspect / texture_aspect, texture_aspect / window_aspect)`，
///   以较大缩放因子填充整个窗口，居中裁剪。
///
/// **Contain 模式**（`fit_mode == 0.0`）：
///   缩放纹理以适应窗口，保持完整画面，可能留黑边。
///   算法：`scale = min(...)`，以较小缩放因子保持完整内容可见。
///
/// # 坐标约定
/// - wgpu 纹理坐标: UV (0,0) = 左上角, (1,1) = 右下角
/// - NDC 屏幕坐标: (-1,-1) = 左下角, (1,1) = 右上角
/// - 顶点着色器已完成 V 翻转，输入 UV 与纹理坐标一致
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // 步骤 1：计算宽高比
    let texture_aspect = u_fit.texture_size.x / u_fit.texture_size.y;
    let window_aspect = u_fit.window_size.x / u_fit.window_size.y;

    // 步骤 2：非均匀 UV 缩放 — 分别计算 U 和 V 方向的缩放范围
    //
    // 核心原理：屏幕像素在 UV 空间的 U/V 方向密度不同（因为窗口不一定正方形），
    // 因此必须分别压缩 U 或 V 才能保持纹理像素在屏幕上为正方形（无拉伸）。
    //
    // Cover 模式：短边填满窗口，长边居中裁切
    //   - 窗口较宽 → 纹理高度填满，左右裁切 → 压缩 V 范围
    //   - 纹理较宽 → 纹理宽度填满，上下裁切 → 压缩 U 范围
    //
    // Contain 模式：长边适配窗口，短边居中留空（黑色）
    //   - 窗口较宽 → 纹理高度填满，左右留黑 → 压缩 U 范围
    //   - 纹理较宽 → 纹理宽度填满，上下留黑 → 压缩 V 范围
    var uv_u_range: f32;
    var uv_v_range: f32;

    if (u_fit.fit_mode > 0.5) {
        // Cover 模式（默认）
        if (window_aspect > texture_aspect) {
            // 窗口比纹理更宽 → 纹理高度填满窗口，U 方向裁切（压缩 V）
            uv_u_range = 1.0;
            uv_v_range = texture_aspect / window_aspect; // < 1，垂直裁切
        } else {
            // 纹理比窗口更宽（或相等）→ 纹理宽度填满窗口，V 方向裁切（压缩 U）
            uv_u_range = window_aspect / texture_aspect; // < 1，水平裁切
            uv_v_range = 1.0;
        }
    } else {
        // Contain 模式 — 纹理完整可见，窗口空白区域为黑色
        if (window_aspect > texture_aspect) {
            // 窗口比纹理更宽 → 纹理高度填满，左右留黑边
            // uv_u_range > 1.0 = "zoom out"：texture U 范围收缩，屏幕 UV 超出 [0,1] 变黑
            uv_u_range = window_aspect / texture_aspect; // > 1，水平收缩留黑
            uv_v_range = 1.0;
        } else {
            // 纹理比窗口更宽 → 纹理宽度填满，上下留黑边
            // uv_v_range > 1.0 = "zoom out"：texture V 范围收缩，屏幕 UV 超出 [0,1] 变黑
            uv_u_range = 1.0;
            uv_v_range = texture_aspect / window_aspect; // > 1，垂直收缩留黑
        }
    }

    // 步骤 3：分别缩放 U 和 V，居中偏移
    let scaled_u = (input.uv.x - 0.5) * uv_u_range + 0.5;
    let scaled_v = (input.uv.y - 0.5) * uv_v_range + 0.5;

    // 步骤 4：对于 contain 模式，UV 超出 [0,1] 的区域显示黑色
    //   （ClampToEdge 会泄露边缘像素颜色，因此显式检测并返回纯黑）
    if (scaled_u < 0.0 || scaled_u > 1.0 || scaled_v < 0.0 || scaled_v > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // 步骤 5：采样纹理
    let color = textureSample(t_background, s_background, vec2<f32>(scaled_u, scaled_v));
    return color;
}
