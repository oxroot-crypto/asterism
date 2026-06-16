//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/renderer_impl.rs
//! 功能概述：真实渲染器实现 — 实现 `Renderer` trait，桥接 CommandBridge 到 `aster-renderer` 的具体类型。
//!           持有 BackgroundLayer / SpriteLayer × 2 / TextRenderer，管理纹理缓存和立绘 ID 映射。
//!           Phase 1 完整实现渲染命令 → GPU 操作的转换链路。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_renderer（BackgroundLayer / SpriteLayer / SpritePosition / TextRenderer / Texture / frame_capture）
//! - aster_save（UiCommand — 存档 UI 渲染指令）
//! - wgpu（Device / Queue / CommandEncoder / TextureView / SurfaceTexture）
//! - crate::command_bridge::Renderer（trait 定义）
//!
//! 对应任务：PH1-T18 — Renderer trait 的真实实现；PH2-T08 — 集成截图/UI 渲染
//! 架构位置：aster-runtime — 依赖 aster-renderer，由 PH1-T21 主事件循环实例化

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use aster_asset::AssetManager;
use aster_renderer::{
    BackgroundLayer, Layer, SpriteDescriptor, SpriteLayer, SpritePosition, TextConfig,
    TextRenderer, Texture,
};
use image::ImageEncoder;
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
    /// 角色→立绘映射：(sprite_id, layer, position_byte)
    /// layer: 0=后层, 1=前层; position_byte: VM 位置编码(0=左,1=中,2=右)
    char_sprites: HashMap<String, (u64, u8, u8)>,
    sprite_ids: HashMap<String, u64>,
    /// 项目根目录（用于解析资产相对路径）
    project_root: std::path::PathBuf,
    /// 屏幕尺寸
    pub screen_width: u32,
    pub screen_height: u32,
    /// PH2-T08: 存档/读档 UI 是否激活
    save_ui_active: bool,
    /// PH2-T08: 存档/读档 UI 格式化后的文本
    save_ui_text: String,
    /// PH2-T09: 暂停菜单是否激活
    pause_menu_active: bool,
    /// PH2-T09: 暂停菜单格式化后的文本
    pause_menu_text: String,
    /// PH2-T08: 资源管理器（用于统一资源加载+LRU 缓存）
    asset_manager: Option<Arc<Mutex<AssetManager>>>,
}

impl GameRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        screen_width: u32,
        screen_height: u32,
        project_root: PathBuf,
        asset_manager: Option<Arc<Mutex<AssetManager>>>,
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
            save_ui_active: false,
            save_ui_text: String::new(),
            pause_menu_active: false,
            pause_menu_text: String::new(),
            asset_manager,
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

        // 文本（叠加层激活时跳过对话渲染，避免文本透过半透明遮罩可见）
        // 保存当前对话状态（叠加层会临时覆盖 text_renderer）
        let (dialogue_speaker, dialogue_body) = {
            let (s, b) = self.text_renderer.current_text();
            (s.to_string(), b.to_string())
        };
        let has_overlay = (self.save_ui_active && !self.save_ui_text.is_empty())
            || (self.pause_menu_active && !self.pause_menu_text.is_empty());

        if !has_overlay {
            self.text_renderer.render(encoder, output_view);
        }

        // PH2-T08: Save UI 叠加层
        // 注意：每帧必须 set_text + prepare，因为上方的 dialogue prepare()
        // 已覆盖字形缓冲区，set_text() 会使旧布局失效，必须重新 prepare。
        if self.save_ui_active && !self.save_ui_text.is_empty() {
            self.text_renderer.set_text("", &self.save_ui_text);
            self.text_renderer.set_visible_range(0, usize::MAX);
            self.text_renderer.prepare(&self.device, &self.queue);
            self.text_renderer.render(encoder, output_view);
        }

        // PH2-T09: 暂停菜单叠加层
        if self.pause_menu_active && !self.pause_menu_text.is_empty() {
            self.text_renderer.set_text("", &self.pause_menu_text);
            self.text_renderer.set_visible_range(0, usize::MAX);
            self.text_renderer.prepare(&self.device, &self.queue);
            self.text_renderer.render(encoder, output_view);
        }

        // 恢复对话文本状态，确保叠加层关闭后下一帧重新排版对话文本
        if has_overlay {
            self.text_renderer
                .restore_dialogue_text(&dialogue_speaker, &dialogue_body);
        }
    }

    /// 从 wgpu SurfaceTexture 截取当前帧，返回降采样后的 RGB 像素数据。
    ///
    /// 在 `render()` 之后、`present()` 之前调用。
    /// 使用 GPU → CPU 回读（copy_texture_to_buffer + map_async），
    /// 对性能有约 10-20ms 影响，仅应在存档等低频操作中使用。
    ///
    /// # 参数
    /// - `surface_texture`: 当前帧的 swapchain 纹理
    /// - `thumbnail_width` / `thumbnail_height`: 缩略图尺寸（默认 320×180）
    ///
    /// # 性能
    /// - GPU 回读开销约 10-20ms（低频操作，存档时使用）
    pub fn capture_from_surface(
        &self,
        surface_texture: &wgpu::SurfaceTexture,
        thumbnail_width: u32,
        thumbnail_height: u32,
    ) -> Result<Vec<u8>, String> {
        let width = self.screen_width;
        let height = self.screen_height;
        let pixel_bytes = (width * height * 4) as u64;

        // 创建 staging buffer 用于 GPU → CPU 回读
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("截图 Staging Buffer"),
            size: pixel_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // 从 surface texture 拷贝到 staging buffer
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("截图 Command Encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &surface_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // 提交并等待 GPU 完成
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);

        // 映射并读取数据
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);

        rx.recv()
            .map_err(|_| "截图: 无法接收 map 结果".to_string())?
            .map_err(|e| format!("截图: buffer map 失败: {}", e))?;

        let data = buffer_slice.get_mapped_range();
        let rgba_pixels: Vec<u8> = data[..pixel_bytes as usize].to_vec();
        drop(data);
        staging_buffer.unmap();

        // 简单降采样到缩略图尺寸（最近邻采样）
        let thumb = Self::downsample_rgba(
            &rgba_pixels,
            width,
            height,
            thumbnail_width,
            thumbnail_height,
        );

        // 编码为 PNG（image 0.25 API: write_image）
        let mut png_bytes = Vec::new();
        {
            let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
            encoder
                .write_image(
                    &thumb,
                    thumbnail_width,
                    thumbnail_height,
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| format!("截图 PNG 编码失败: {}", e))?;
        }

        Ok(png_bytes)
    }

    /// 最近邻降采样 RGBA 像素到目标尺寸。
    fn downsample_rgba(
        pixels: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity((dst_width * dst_height * 4) as usize);
        for dy in 0..dst_height {
            let sy = (dy as f64 * src_height as f64 / dst_height as f64) as u32;
            for dx in 0..dst_width {
                let sx = (dx as f64 * src_width as f64 / dst_width as f64) as u32;
                let idx = ((sy * src_width + sx) * 4) as usize;
                out.extend_from_slice(&pixels[idx..idx + 4]);
            }
        }
        out
    }

    /// PH2-T08: 设置资源管理器（在 AssetManager 初始化后调用）。
    pub fn set_asset_manager(&mut self, mgr: Arc<Mutex<AssetManager>>) {
        self.asset_manager = Some(mgr);
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

    /// 加载纹理 — 优先通过 AssetManager（含 LRU 缓存），fallback 到直接文件加载。
    ///
    /// 路径相对于项目根目录（如 "assets/bg/classroom.png"）。
    ///
    /// # 缓存层级说明
    ///
    /// 纹理存在两级缓存：
    /// 1. AssetManager LRU 缓存（跨子系统共享，持有 wgpu Texture）
    /// 2. 本地 `texture_cache`（HashMap<String, Texture>，持有 aster-renderer Texture 包装）
    ///
    /// 两级缓存各司其职：AssetManager 管理原始 GPU 纹理的生命周期和预算，
    /// 本地缓存避免每帧重新创建 `Texture` 包装和 `TextureView`。
    /// wgpu Texture 内部使用 Arc，clone 开销极低。
    ///
    /// 已知限制：当 `set_background()` 从本地缓存 remove 纹理时，
    /// AssetManager 中对应的 wgpu Texture 引用仍保留。
    /// 这导致 AssetManager 的预算计数器略微偏高，但不浪费 GPU 内存（Arc 共享）。
    /// 未来可添加 `AssetManager::release()` 方法通知 LRU 缓存释放引用。
    fn load_texture(&mut self, path: &str) -> Option<&Texture> {
        // 步骤 1：本地缓存
        if self.texture_cache.contains_key(path) {
            return self.texture_cache.get(path);
        }

        // 步骤 2：通过 AssetManager 加载（含 LRU 缓存）
        if let Some(ref asset_mgr) = self.asset_manager {
            // Mutex poison 恢复：若其他线程 panic 导致锁中毒，仍可安全取回内部数据。
            // AssetManager 不变量由单线程（事件循环）保证，中毒不会破坏数据一致性。
            let mut mgr = asset_mgr
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let path_std = std::path::Path::new(path);
            if let Some(asset_id) = mgr.find_by_path(path_std)
                && let Ok(cached) = mgr.load(asset_id)
                && let aster_asset::LoadedAsset::Texture { size, texture, .. } = &cached.data
            {
                let gpu_tex = texture;
                let gpu_view = gpu_tex.create_view(&wgpu::TextureViewDescriptor::default());
                let tex = Texture::from_wgpu_texture(
                    &self.device,
                    gpu_tex.clone(),
                    gpu_view,
                    size.0,
                    size.1,
                    Some(path),
                );
                let hit_rate = mgr.stats().hit_rate() * 100.0;
                log::debug!(
                    "[GameRenderer] AssetManager: {} (命中率 {:.0}%)",
                    path,
                    hit_rate
                );
                self.texture_cache.insert(path.to_string(), tex);
                return self.texture_cache.get(path);
            }
        }

        // 步骤 3：直接文件加载（fallback）
        let full_path = self.project_root.join(path);
        let texture = Texture::from_file(&self.device, &self.queue, &full_path, None).ok()?;
        self.texture_cache.insert(path.to_string(), texture);
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
            // 从本地缓存取出纹理（所有权转移给 BackgroundLayer）。
            // 注意：AssetManager LRU 缓存仍持有 wgpu Texture 引用，
            // 但因为 wgpu Texture 内部为 Arc，不会造成显存浪费。
            // 详见 load_texture() 的"缓存层级说明"注释。
            if let Some(texture) = self.texture_cache.remove(path) {
                self.bg_layer.set_background(&self.queue, texture);
                // 重新以 id 为 key 插入缓存（set_background 后 BG 纹理由 bg_layer 拥有）
                // 实际上 Texture 被 move 进 bg_layer，缓存中不再持有
                // 如果后续需要重新加载，从磁盘再次读取
            }
        }
    }

    fn show_character(&mut self, char_id: &str, sprite_path: &str, position: u8) {
        // 步骤 1：先加载新纹理（在移除旧精灵之前，保证原子性）
        let texture = match Texture::from_file(
            &self.device,
            &self.queue,
            self.project_root.join(sprite_path),
            None,
        ) {
            Ok(t) => t,
            Err(_) => return, // 加载失败，保留旧立绘不变
        };

        // 步骤 2：纹理加载成功后，移除旧立绘
        if let Some(&(old_id, old_layer, _)) = self.char_sprites.get(char_id) {
            if old_layer == 0 {
                self.sprite_back.remove_sprite(old_id);
            } else {
                self.sprite_front.remove_sprite(old_id);
            }
        }

        // 步骤 3：添加新立绘
        let sprite_pos = Self::pos_byte_to_sprite_position(position);
        let desc = SpriteDescriptor::new(sprite_pos);

        let sprite_id = self
            .sprite_front
            .add_sprite(&self.device, &self.queue, texture, desc);

        self.char_sprites
            .insert(char_id.to_string(), (sprite_id, 1, position)); // layer=1(front), 保存位置
    }

    fn hide_character(&mut self, char_id: &str) {
        if let Some(&(sprite_id, layer, _)) = self.char_sprites.get(char_id) {
            if layer == 0 {
                self.sprite_back.remove_sprite(sprite_id);
            } else {
                self.sprite_front.remove_sprite(sprite_id);
            }
            self.char_sprites.remove(char_id);
        }
    }

    fn move_character(&mut self, char_id: &str, position: u8) {
        // 使用 SpriteLayer::update_position 原地更新位置，无需移除/重新添加
        if let Some(&(sprite_id, layer, old_position)) = self.char_sprites.get(char_id) {
            if position == old_position {
                return; // 位置未变，跳过
            }
            let sprite_pos = Self::pos_byte_to_sprite_position(position);
            if layer == 0 {
                self.sprite_back
                    .update_position(&self.queue, sprite_id, sprite_pos);
            } else {
                self.sprite_front
                    .update_position(&self.queue, sprite_id, sprite_pos);
            }
            // 更新位置记录
            self.char_sprites
                .insert(char_id.to_string(), (sprite_id, layer, position));
        }
    }

    fn set_emotion(&mut self, char_id: &str, sprite_path: &str) {
        // 步骤 1：保存当前角色的位置和图层信息
        let (old_id, layer, position) = match self.char_sprites.get(char_id).copied() {
            Some(info) => info,
            None => return, // 角色未显示，直接返回
        };

        // 步骤 2：先加载新纹理（在移除旧精灵之前，保证原子性）
        let texture = match Texture::from_file(
            &self.device,
            &self.queue,
            self.project_root.join(sprite_path),
            None,
        ) {
            Ok(t) => t,
            Err(_) => return, // 加载失败，保留旧立绘
        };

        // 步骤 3：移除旧立绘
        if layer == 0 {
            self.sprite_back.remove_sprite(old_id);
        } else {
            self.sprite_front.remove_sprite(old_id);
        }

        // 步骤 4：添加新立绘，保持原位置
        let sprite_pos = Self::pos_byte_to_sprite_position(position);
        let desc = SpriteDescriptor::new(sprite_pos);

        let sprite_id = if layer == 0 {
            self.sprite_back
                .add_sprite(&self.device, &self.queue, texture, desc)
        } else {
            self.sprite_front
                .add_sprite(&self.device, &self.queue, texture, desc)
        };

        self.char_sprites
            .insert(char_id.to_string(), (sprite_id, layer, position));
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

    // ─── PH2-T08 新增方法 ────────────────────────────────────────────

    fn capture_screenshot(&self) -> Result<Vec<u8>, String> {
        // 渲染器内部的简单截图 — 依赖于外部（App）在渲染时提供 SurfaceTexture
        // 实际截图由 GameRenderer::capture_from_surface() 完成，
        // 此方法保留用于 MockRenderer 等无 GPU 场景。
        Err("截图功能需在 App 层通过 capture_from_surface() 调用".into())
    }

    fn render_save_ui(&mut self, commands: &[aster_save::UiCommand]) {
        self.save_ui_active = true;
        // 将 UiCommand 列表格式化为可显示的文本
        let mut text = String::new();
        for cmd in commands {
            match cmd {
                aster_save::UiCommand::Overlay { .. } => {
                    // 半透明遮罩 — Phase 2 仅文本渲染，跳过视觉遮罩
                }
                aster_save::UiCommand::Text {
                    content, selected, ..
                } => {
                    if *selected {
                        text.push_str("> ");
                    } else {
                        text.push_str("  ");
                    }
                    text.push_str(content);
                    text.push('\n');
                }
                aster_save::UiCommand::Thumbnail { .. } => {
                    // 缩略图 — Phase 2 跳过（无 UI 纹理渲染）
                    text.push_str("  [缩略图]\n");
                }
                aster_save::UiCommand::ConfirmDialog { message, .. } => {
                    text.push_str("  ╔══════════════════╗\n");
                    text.push_str(&format!("  ║ {}\n", message));
                    text.push_str("  ╚══════════════════╝\n");
                }
            }
        }
        self.save_ui_text = text;
    }

    fn clear_save_ui(&mut self) {
        self.save_ui_active = false;
        self.save_ui_text.clear();
    }

    fn render_pause_menu(&mut self, commands: &[aster_save::UiCommand]) {
        self.pause_menu_active = true;
        // 将 UiCommand 列表格式化为可显示的文本
        let mut text = String::new();
        for cmd in commands {
            if let aster_save::UiCommand::Text {
                content, selected, ..
            } = cmd
            {
                if *selected {
                    text.push_str("> ");
                } else {
                    text.push_str("  ");
                }
                text.push_str(content);
                text.push('\n');
            }
        }
        self.pause_menu_text = text;
    }

    fn clear_pause_menu(&mut self) {
        self.pause_menu_active = false;
        self.pause_menu_text.clear();
    }

    fn clear_all_characters(&mut self) {
        // 收集所有角色 ID 和精灵 ID（避免借用在迭代期间冲突）
        let char_ids: Vec<String> = self.char_sprites.keys().cloned().collect();
        let sprite_paths: Vec<String> = self.sprite_ids.keys().cloned().collect();
        // 逐一隐藏角色
        for char_id in &char_ids {
            self.hide_character(char_id);
        }
        // 清除通用精灵（不使用 hide_sprite 因为需要特定实现对精灵存储的访问）
        for path in &sprite_paths {
            if let Some(&sprite_id) = self.sprite_ids.get(path) {
                self.sprite_back.remove_sprite(sprite_id);
                self.sprite_front.remove_sprite(sprite_id);
            }
        }
        self.sprite_ids.clear();
        self.char_sprites.clear();
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
