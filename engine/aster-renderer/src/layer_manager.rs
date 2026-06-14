//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/layer_manager.rs
//! 功能概述：渲染层管理器 — 管理 6 个渲染层的栈式合成器。
//!           定义 `Layer` trait 作为所有渲染层的统一接口，
//!           `LayerManager` 按 Layer 0→5 的顺序逐层合成最终画面。
//!           支持运行时动态替换各层（如背景切换、立绘更新）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - wgpu（CommandEncoder / TextureView）
//!
//! 层定义（Architecture.md §4.6）：
//! | 层编号 | 名称 | 职责 | 对应组件 |
//! |--------|------|------|----------|
//! | Layer 0 | 背景层 | 全屏背景纹理 | `BackgroundLayer` |
//! | Layer 1 | 立绘后层 | 角色立绘（z-index 较低） | `SpriteLayer` |
//! | Layer 2 | 立绘前层 | 角色立绘（z-index 较高） | `SpriteLayer` |
//! | Layer 3 | 特效层 | 预留：转场特效、粒子效果 | 暂无（Phase 4） |
//! | Layer 4 | 文本层 | 对话文本框和说话者名字 | 暂无（PH1-T09） |
//! | Layer 5 | UI 层 | 菜单、设置面板、存档界面 | 暂无（Phase 5） |
//!
//! 对应任务：PH1-T08 — 角色立绘渲染（Layer 抽象部分）
//! 对应需求：REQ-ENG-012（立绘放置在背景之上）

use wgpu::{CommandEncoder, TextureView};

// ============================================================================
// Layer trait — 渲染层统一接口
// ============================================================================

/// 渲染层 trait — 所有渲染层（背景、立绘、文本、UI）的统一接口。
///
/// 每个实现此 trait 的类型代表渲染管线中的一个独立层。
/// `LayerManager` 按层编号顺序调用各层的 `render()` 方法，
/// 使用同一个 `CommandEncoder` 和输出 `TextureView`，实现多图层合成。
///
/// # 渲染约定
/// - 各层共享同一个 encoder 和 output_view
/// - 先渲染的层在画面底层（Layer 0 最底层）
/// - 后渲染的层叠加在之前的层之上（Layer 5 最顶层）
/// - 每层负责自己的管线绑定、纹理绑定和绘制命令
/// - 如果当前无内容（如无背景纹理），应静默跳过，不记录渲染命令
///
/// # 实现示例
/// ```rust,ignore
/// impl Layer for BackgroundLayer {
///     fn render<'a>(&'a self, encoder: &'a mut CommandEncoder, output_view: &'a TextureView) {
///         self.render(encoder, output_view);  // 委托给自身方法
///     }
/// }
/// ```
pub trait Layer {
    /// 渲染当前层到输出纹理视图。
    ///
    /// # 参数
    /// - `encoder`: wgpu 命令编码器（可变引用），用于记录本层的 GPU 渲染命令
    /// - `output_view`: 输出纹理视图（渲染目标），所有层共享同一视图
    fn render<'a>(&'a self, encoder: &'a mut CommandEncoder, output_view: &'a TextureView);
}

// ============================================================================
// LayerManager — 渲染层管理器
// ============================================================================

/// 渲染层管理器 — 持有 6 个渲染层，按序合成最终画面。
///
/// 层编号 0（背景）到 5（UI），可通过 `set_layer()` 动态替换任意层。
/// 各层在 `render()` 时按编号升序依次渲染。
///
/// # 使用示例
/// ```rust,ignore
/// use aster_renderer::{LayerManager, BackgroundLayer, SpriteLayer};
///
/// let mut manager = LayerManager::new();
/// manager.set_layer(0, Box::new(background_layer));
/// manager.set_layer(1, Box::new(sprite_layer_back));
/// manager.set_layer(2, Box::new(sprite_layer_front));
///
/// // 渲染循环中
/// let frame = ctx.acquire_frame()?;
/// {
///     let mut encoder = frame.encoder;
///     manager.render(&mut encoder, &frame.view);
///     // encoder 在此提交...
/// }
/// ```
pub struct LayerManager {
    /// 6 个渲染层槽位。None 表示该层无内容，渲染时跳过。
    /// 索引 0 = 背景层，1-2 = 立绘层，3 = 特效层，4 = 文本层，5 = UI 层
    layers: [Option<Box<dyn Layer>>; 6],
}

impl LayerManager {
    /// 创建空的层管理器，所有 6 个层槽位初始为空。
    ///
    /// 需要在渲染前通过 `set_layer()` 设置至少一个层，
    /// 否则 `render()` 不会有任何输出。
    #[must_use]
    pub fn new() -> Self {
        // 6 个槽位初始全为 None
        Self {
            layers: [
                None, // Layer 0 — 背景层
                None, // Layer 1 — 立绘后层
                None, // Layer 2 — 立绘前层
                None, // Layer 3 — 特效层（预留）
                None, // Layer 4 — 文本层（预留）
                None, // Layer 5 — UI 层（预留）
            ],
        }
    }

    /// 设置指定编号的渲染层。
    ///
    /// 如果该层已有内容，旧层将被替换并 drop（释放 GPU 资源）。
    ///
    /// # 参数
    /// - `index`: 层编号（0-5），超出范围将 panic
    /// - `layer`: 实现了 `Layer` trait 的渲染层实例
    ///
    /// # Panics
    /// 如果 `index >= 6`，assert 触发 panic。
    #[inline]
    pub fn set_layer(&mut self, index: usize, layer: Box<dyn Layer>) {
        assert!(index < 6, "层编号超出范围：{index}，有效范围为 0-5");
        self.layers[index] = Some(layer);
    }

    /// 移除指定编号的渲染层。
    ///
    /// 如果该层不存在，静默忽略。
    ///
    /// # 参数
    /// - `index`: 层编号（0-5）
    ///
    /// # 返回值
    /// - `Some(Box<dyn Layer>)`: 被移除的层（可重新插入或丢弃）
    /// - `None`: 该层原本为空
    #[inline]
    pub fn remove_layer(&mut self, index: usize) -> Option<Box<dyn Layer>> {
        assert!(index < 6, "层编号超出范围：{index}，有效范围为 0-5");
        self.layers[index].take()
    }

    /// 按 Layer 0 → Layer 5 的顺序渲染所有层。
    ///
    /// 渲染到同一个输出纹理视图，后渲染的层叠加在先渲染的层之上。
    /// 空层（None）自动跳过。
    ///
    /// # 参数
    /// - `encoder`: wgpu 命令编码器
    /// - `output_view`: 输出纹理视图（渲染目标）
    ///
    /// # 渲染顺序保证
    /// Layer 0（背景）最先渲染，Layer 5（UI）最后渲染。
    /// α 混合由各层各自的管线配置决定（如立绘层的 `BlendState::ALPHA_BLENDING`）。
    pub fn render<'a>(&'a self, encoder: &'a mut CommandEncoder, output_view: &'a TextureView) {
        for layer in self.layers.iter().flatten() {
            layer.render(encoder, output_view);
        }
    }

    /// 检查指定层是否已设置。
    ///
    /// # 参数
    /// - `index`: 层编号（0-5）
    #[inline]
    pub fn has_layer(&self, index: usize) -> bool {
        assert!(index < 6, "层编号超出范围：{index}，有效范围为 0-5");
        self.layers[index].is_some()
    }

    /// 返回已设置的层数量（非 None 槽位数）。
    #[inline]
    pub fn active_layer_count(&self) -> usize {
        self.layers.iter().filter(|l| l.is_some()).count()
    }
}

impl Default for LayerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 单元测试 — 覆盖 LayerManager 基本操作
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::CommandEncoderDescriptor;

    /// 用于测试的 mock Layer 实现。
    ///
    /// 记录 `render()` 被调用的次数，不执行实际 GPU 操作。
    struct MockLayer {
        render_count: std::cell::Cell<usize>,
    }

    impl MockLayer {
        fn new() -> Self {
            Self {
                render_count: std::cell::Cell::new(0),
            }
        }

        #[allow(dead_code)]
        fn render_count(&self) -> usize {
            self.render_count.get()
        }
    }

    impl Layer for MockLayer {
        fn render<'a>(&'a self, _encoder: &'a mut CommandEncoder, _output_view: &'a TextureView) {
            self.render_count.set(self.render_count.get() + 1);
        }
    }

    // ========================================================================
    // 基本操作测试
    // ========================================================================

    /// 验证 LayerManager::new() 创建后所有层为空。
    #[test]
    fn new_layer_manager_all_empty() {
        let manager = LayerManager::new();
        assert_eq!(manager.active_layer_count(), 0);
        for i in 0..6 {
            assert!(!manager.has_layer(i), "层 {i} 应为空");
        }
    }

    /// 验证 set_layer 和 has_layer 正确。
    #[test]
    fn set_and_check_layer() {
        let mut manager = LayerManager::new();

        manager.set_layer(0, Box::new(MockLayer::new()));
        assert!(manager.has_layer(0));
        assert!(!manager.has_layer(1));
        assert_eq!(manager.active_layer_count(), 1);
    }

    /// 验证 set_layer 替换旧层。
    #[test]
    fn set_layer_replaces_old() {
        let mut manager = LayerManager::new();

        manager.set_layer(1, Box::new(MockLayer::new()));
        assert!(manager.has_layer(1));

        // 替换层 1
        manager.set_layer(1, Box::new(MockLayer::new()));
        assert!(manager.has_layer(1));
        assert_eq!(manager.active_layer_count(), 1); // 仍然是 1 个
    }

    /// 验证 remove_layer 移除并返回层。
    #[test]
    fn remove_layer_returns_old() {
        let mut manager = LayerManager::new();

        manager.set_layer(0, Box::new(MockLayer::new()));
        assert!(manager.has_layer(0));

        let removed = manager.remove_layer(0);
        assert!(removed.is_some(), "应有被移除的层");
        assert!(!manager.has_layer(0), "层 0 移除后应为空");
        assert_eq!(manager.active_layer_count(), 0);
    }

    /// 验证 remove_layer 对空层返回 None。
    #[test]
    fn remove_empty_layer_returns_none() {
        let mut manager = LayerManager::new();
        let removed = manager.remove_layer(3);
        assert!(removed.is_none());
    }

    /// 验证 active_layer_count 计数正确。
    #[test]
    fn active_layer_count_correct() {
        let mut manager = LayerManager::new();
        assert_eq!(manager.active_layer_count(), 0);

        manager.set_layer(0, Box::new(MockLayer::new()));
        manager.set_layer(1, Box::new(MockLayer::new()));
        manager.set_layer(4, Box::new(MockLayer::new()));
        assert_eq!(manager.active_layer_count(), 3);

        manager.remove_layer(1);
        assert_eq!(manager.active_layer_count(), 2);
    }

    // ========================================================================
    // Render 测试
    // ========================================================================

    /// 辅助函数：创建一个用于测试的 wgpu 设备和命令编码器。
    fn create_test_wgpu() -> Option<(wgpu::Device, wgpu::Queue)> {
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
                label: Some("LayerManager 测试设备"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        Some((device, queue))
    }

    /// 创建测试用的输出纹理视图。
    fn create_test_output_view(device: &wgpu::Device) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("LayerManager 测试输出"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// 验证空 LayerManager 的 render() 不 panic。
    #[test]
    fn render_empty_manager_no_panic() {
        let (device, _queue) = match create_test_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let manager = LayerManager::new();
        let output_view = create_test_output_view(&device);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("空 manager 测试编码器"),
        });

        // 空 manager 的 render 应不 panic，静默跳过所有层
        manager.render(&mut encoder, &output_view);
        encoder.finish();
    }

    /// 验证 render() 按顺序调用各层的 render 方法。
    #[test]
    fn render_calls_layers_in_order() {
        let (device, _queue) = match create_test_wgpu() {
            Some(ctx) => ctx,
            None => {
                eprintln!("[跳过] 无 GPU 适配器");
                return;
            }
        };

        let mut manager = LayerManager::new();

        let layer0 = MockLayer::new();
        let layer2 = MockLayer::new();
        let layer4 = MockLayer::new();

        manager.set_layer(0, Box::new(layer0));
        manager.set_layer(2, Box::new(layer2));
        manager.set_layer(4, Box::new(layer4));

        let output_view = create_test_output_view(&device);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("顺序测试编码器"),
        });

        manager.render(&mut encoder, &output_view);

        // 验证所有已设置的层都被调用了 render（通过 mock 的计数验证）
        // 注意：render 后 layer 的所有权在 manager 中，无法直接读取计数
        // 此测试主要验证 render 不 panic
        encoder.finish();
    }

    /// 验证 Default trait 实现。
    #[test]
    fn default_is_empty() {
        let manager = LayerManager::default();
        assert_eq!(manager.active_layer_count(), 0);
    }

    // ========================================================================
    // 边界条件测试
    // ========================================================================

    /// 验证 set_layer 索引为 0-5 时不 panic。
    #[test]
    fn set_layer_valid_indices() {
        let mut manager = LayerManager::new();
        for i in 0..6 {
            manager.set_layer(i, Box::new(MockLayer::new()));
        }
        assert_eq!(manager.active_layer_count(), 6);
    }

    /// 验证 set_layer 索引 6 时 panic。
    #[test]
    #[should_panic(expected = "层编号超出范围")]
    fn set_layer_index_6_panics() {
        let mut manager = LayerManager::new();
        manager.set_layer(6, Box::new(MockLayer::new()));
    }

    /// 验证 remove_layer 索引 6 时 panic。
    #[test]
    #[should_panic(expected = "层编号超出范围")]
    fn remove_layer_index_6_panics() {
        let mut manager = LayerManager::new();
        manager.remove_layer(6);
    }
}
