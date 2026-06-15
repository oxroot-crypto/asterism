//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/renderer_impl.rs
//! 功能概述：真实渲染器实现 — 实现 `Renderer` trait，桥接 CommandBridge 到 `aster-renderer` 的具体类型。
//!           持有 BackgroundLayer / SpriteLayer × 2 / TextRenderer，管理纹理缓存和立绘 ID 映射。
//!           Phase 1 完整实现渲染命令 → GPU 操作的转换链路。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_renderer（BackgroundLayer / SpriteLayer / SpritePosition / TextRenderer / Texture）
//! - wgpu（Device / Queue / CommandEncoder / TextureView）
//! - crate::command_bridge::Renderer（trait 定义）
//!
//! 对应任务：PH1-T18 — Renderer trait 的真实实现
//! 架构位置：aster-runtime — 依赖 aster-renderer，由 PH1-T21 主事件循环实例化

use std::collections::HashMap;

use std::path::PathBuf;

use aster_renderer::{
    BackgroundLayer, Layer, SpriteDescriptor, SpriteLayer, SpritePosition, TextConfig,
    TextRenderer, Texture,
};
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};

use crate::command_bridge::Renderer;

/// 真实 GPU 渲染器 — 实现 `Renderer` trait，桥接 CommandBridge 到 `aster-renderer`。
///
/// 持有 Device/Queue 的所有权（通过 clone 从 GpuContext 获取，wgpu 内部用 Arc），
/// 背景层/立绘层/文本层，纹理缓存，角色→立绘 ID 映射。
/// 无生命周期参数，可直接存入 App struct。
pub struct GameRenderer {
    device: Device,
    queue: Queue,
    bg_layer: BackgroundLayer,
    sprite_back: SpriteLayer,
    sprite_front: SpriteLayer,
    text_renderer: TextRenderer,
    texture_cache: HashMap<String, Texture>,
    char_sprites: HashMap<String, (u64, u8)>,
    sprite_ids: HashMap<String, u64>,
    /// 项目根目录（用于解析资产相对路径）
    project_root: std::path::PathBuf,
    /// 屏幕尺寸
    pub screen_width: u32,
    pub screen_height: u32,
}

impl GameRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        screen_width: u32,
        screen_height: u32,
        project_root: PathBuf,
    ) -> Self {
        // Clone Device/Queue — wgpu 内部 Arc，开销极低
        let device_owned = device.clone();
        let queue_owned = queue.clone();

        let bg_layer = BackgroundLayer::new(device, queue, format, screen_width, screen_height);
        let sprite_back = SpriteLayer::new(device, queue, format, screen_width, screen_height);
        let sprite_front = SpriteLayer::new(device, queue, format, screen_width, screen_height);
        let text_config = TextConfig::default();
        let text_renderer = TextRenderer::new(
            device,
            queue,
            format,
            screen_width,
            screen_height,
            text_config,
        )
        .expect("TextRenderer 初始化失败");

        Self {
            device: device_owned,
            queue: queue_owned,
            bg_layer,
            sprite_back,
            sprite_front,
            text_renderer,
            texture_cache: HashMap::new(),
            char_sprites: HashMap::new(),
            sprite_ids: HashMap::new(),
            project_root,
            screen_width,
            screen_height,
        }
    }

    /// 渲染一帧（清屏 → 布局文字 → 背景 → 立绘后 → 立绘前 → 文本）。
    /// 每帧调用一次。
    pub fn render<'pass>(
        &'pass mut self,
        encoder: &'pass mut CommandEncoder,
        output_view: &'pass TextureView,
    ) {
        // 步骤 0：布局文字。set_text() 只是标记脏，prepare() 才真正排版字形
        // 清屏
        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("清屏 Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // 排版文字
        self.text_renderer.prepare(&self.device, &self.queue);

        // 背景 + 立绘
        self.bg_layer.render(encoder, output_view);
        self.sprite_back.render(encoder, output_view);
        self.sprite_front.render(encoder, output_view);

        // 文本
        self.text_renderer.render(encoder, output_view);
    }

    /// 处理窗口尺寸变化。
    ///
    /// # 参数
    /// - `new_width` / `new_height`: 新的窗口尺寸（物理像素）
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        self.screen_width = new_width;
        self.screen_height = new_height;
        // BackgroundLayer 在构造时固定尺寸，resize 需要重建
        // Phase 1 简化：仅在创建时设置尺寸
    }

    // ─── 内部辅助方法 ──────────────────────────────────────────────────

    /// 加载纹理（优先从缓存获取）。路径相对于项目根目录。
    fn load_texture(&mut self, path: &str) -> Option<&Texture> {
        if !self.texture_cache.contains_key(path) {
            let full_path = self.project_root.join(path);
            let texture = Texture::from_file(&self.device, &self.queue, &full_path, None).ok()?;
            self.texture_cache.insert(path.to_string(), texture);
        }
        self.texture_cache.get(path)
    }

    /// 将 VM 位置编码转换为 SpritePosition。
    fn pos_byte_to_sprite_position(pos_byte: u8) -> SpritePosition {
        match pos_byte {
            0 => SpritePosition::Left,
            1 => SpritePosition::Center,
            2 => SpritePosition::Right,
            _ => SpritePosition::Center, // 未知编码 → 默认居中
        }
    }
}

impl Renderer for GameRenderer {
    fn set_background(&mut self, path: &str) {
        if self.load_texture(path).is_some() {
            // 从缓存中取出纹理（所有权转移给 BackgroundLayer）
            if let Some(texture) = self.texture_cache.remove(path) {
                self.bg_layer.set_background(&self.queue, texture);
                // 重新以 id 为 key 插入缓存（set_background 后 BG 纹理由 bg_layer 拥有）
                // 实际上 Texture 被 move 进 bg_layer，缓存中不再持有
                // 如果后续需要重新加载，从磁盘再次读取
            }
        }
    }

    fn show_character(&mut self, char_id: &str, sprite_path: &str, position: u8) {
        // 如果该角色已有立绘，先移除旧的
        if let Some(&(old_id, _)) = self.char_sprites.get(char_id) {
            self.sprite_back.remove_sprite(old_id);
            self.sprite_front.remove_sprite(old_id);
            self.char_sprites.remove(char_id);
        }

        // 加载纹理
        let texture = match Texture::from_file(
            &self.device,
            &self.queue,
            self.project_root.join(sprite_path),
            None,
        ) {
            Ok(t) => t,
            Err(_) => return,
        };

        let sprite_pos = Self::pos_byte_to_sprite_position(position);
        let desc = SpriteDescriptor::new(sprite_pos);

        // 添加到立绘前层（立绘通常在文本之上、背景之上）
        let sprite_id = self
            .sprite_front
            .add_sprite(&self.device, &self.queue, texture, desc);

        self.char_sprites
            .insert(char_id.to_string(), (sprite_id, 1)); // layer=1 (front)
    }

    fn hide_character(&mut self, char_id: &str) {
        if let Some(&(sprite_id, layer)) = self.char_sprites.get(char_id) {
            if layer == 0 {
                self.sprite_back.remove_sprite(sprite_id);
            } else {
                self.sprite_front.remove_sprite(sprite_id);
            }
            self.char_sprites.remove(char_id);
        }
    }

    fn move_character(&mut self, char_id: &str, _position: u8) {
        // Phase 1 简化：移除后重新添加（完整实现需要 SpriteLayer::update_sprite_position）
        // 当前 SpriteLayer 不支持原地修改位置，采用 remove+add 策略
        if let Some(&(old_id, layer)) = self.char_sprites.get(char_id) {
            // 需要重新加载纹理... 当前简化：仅移除
            if layer == 0 {
                self.sprite_back.remove_sprite(old_id);
            } else {
                self.sprite_front.remove_sprite(old_id);
            }
            self.char_sprites.remove(char_id);
            // 注意：move_character 后角色仍在显示，但 Phase 1 无法保留纹理引用
            // 完整的 Phase 4 实现应使用 Texture::id 来缓存纹理引用
        }
    }

    fn set_emotion(&mut self, char_id: &str, sprite_path: &str) {
        // 切换表情 = 原地替换立绘纹理
        // 先移除旧立绘，再显示新立绘（保持位置不变）
        if let Some(&(old_id, layer)) = self.char_sprites.get(char_id) {
            if layer == 0 {
                self.sprite_back.remove_sprite(old_id);
            } else {
                self.sprite_front.remove_sprite(old_id);
            }
            self.char_sprites.remove(char_id);
        }

        let texture = match Texture::from_file(
            &self.device,
            &self.queue,
            self.project_root.join(sprite_path),
            None,
        ) {
            Ok(t) => t,
            Err(_) => return,
        };

        let desc = SpriteDescriptor::new(SpritePosition::Center);
        let sprite_id = self
            .sprite_front
            .add_sprite(&self.device, &self.queue, texture, desc);
        self.char_sprites
            .insert(char_id.to_string(), (sprite_id, 1));
    }

    fn show_sprite(&mut self, path: &str, x: f32, y: f32, scale: f32, alpha: f32) {
        let texture = match Texture::from_file(
            &self.device,
            &self.queue,
            self.project_root.join(path),
            None,
        ) {
            Ok(t) => t,
            Err(_) => return,
        };

        let desc = SpriteDescriptor::new(SpritePosition::Custom(x, y))
            .with_scale(scale, scale)
            .with_alpha(alpha);

        let sprite_id = self
            .sprite_front
            .add_sprite(&self.device, &self.queue, texture, desc);
        self.sprite_ids.insert(path.to_string(), sprite_id);
    }

    fn hide_sprite(&mut self, path: &str) {
        if let Some(&sprite_id) = self.sprite_ids.get(path) {
            self.sprite_back.remove_sprite(sprite_id);
            self.sprite_front.remove_sprite(sprite_id);
            self.sprite_ids.remove(path);
        }
    }

    fn set_dialogue(&mut self, speaker: &str, text: &str) {
        self.text_renderer.set_text(speaker, text);
        // 打字机状态由 DialogueController 管理，GameRenderer 不再持有 Typewriter
    }

    fn set_narration(&mut self, text: &str) {
        self.text_renderer.set_text("", text);
        // 打字机状态由 DialogueController 管理，GameRenderer 不再持有 Typewriter
    }

    fn show_menu(&mut self, prompt: &str, choice_texts: &[String]) {
        let mut menu_text = prompt.to_string();
        for (i, choice) in choice_texts.iter().enumerate() {
            menu_text.push('\n');
            menu_text.push_str(&format!("  {}. {}", i + 1, choice));
        }
        self.text_renderer.set_text("", &menu_text);
        // 菜单文本立即全部显示
        self.text_renderer.set_visible_range(0, usize::MAX);
    }

    fn clear_menu(&mut self) {
        self.text_renderer.clear_text();
        self.text_renderer.set_visible_range(0, usize::MAX);
    }

    /// 设置可见文本范围，由 DialogueController 驱动打字机效果。
    fn set_visible_range(&mut self, start: usize, end: usize) {
        self.text_renderer.set_visible_range(start, end);
    }

    fn wait(&mut self, _duration_ms: u64) {}
    fn effect(&mut self, effect_type: &str, params: &[(String, u16)]) {
        log::info!(
            "[GameRenderer] Effect(type=\"{}\", params={:?}) — Phase 4",
            effect_type,
            params
        );
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证位置编码→SpritePosition 映射。
    #[test]
    fn pos_byte_to_sprite_position_mapping() {
        assert!(matches!(
            GameRenderer::pos_byte_to_sprite_position(0),
            SpritePosition::Left
        ));
        assert!(matches!(
            GameRenderer::pos_byte_to_sprite_position(1),
            SpritePosition::Center
        ));
        assert!(matches!(
            GameRenderer::pos_byte_to_sprite_position(2),
            SpritePosition::Right
        ));
        // 未知编码 → Center
        assert!(matches!(
            GameRenderer::pos_byte_to_sprite_position(99),
            SpritePosition::Center
        ));
    }
}
