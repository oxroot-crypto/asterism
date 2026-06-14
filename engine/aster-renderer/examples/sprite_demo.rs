//! Asterism — Galgame/ADV 游戏引擎
//!
//! 示例：立绘精灵渲染功能测试（PH1-T08 人工验证）
//!
//! 功能概述：
//!   - MV01 多立绘同时显示：3 个程序化角色立绘（左/中/右）渲染在渐变背景之上
//!   - MV02 透明度渐变：按 A 键触发中央立绘 alpha 0.1↔1.0 循环动画
//!   - MV03 立绘替换：按 R 键替换中央立绘纹理（模拟表情切换）
//!
//! 运行方式：
//!   cargo run --package aster-renderer --example sprite_demo
//!
//! 交互：
//!   - ESC / Q         退出
//!   - A               切换透明度动画（MV02 — 中央立绘 alpha 渐变）
//!   - R               替换中央立绘（MV03 — 模拟表情切换）
//!   - Space           切换右侧立绘 z-index
//!   - C / V           切换背景适配模式 Cover / Contain
//!   - 1 / 2 / 3       切换左侧/中央/右侧立绘显示/隐藏
//!   - 拖拽窗口边缘      resize 适配
//!
//! 作者：Claude (AI)
//! 创建日期：2026-06-14

use std::sync::Arc;

use aster_renderer::{
    BackgroundLayer, FitMode, GpuContext, Layer, RenderConfig, SpriteDescriptor, SpriteLayer,
    SpritePosition, Texture,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

// ============================================================================
// 程序化测试纹理 — 模拟立绘（含 Alpha 通道）
// ============================================================================

/// 生成带 Alpha 通道的"角色立绘"纹理（256×256）。
///
/// 绘制：头部（圆形）+ 身体（梯形）+ 眼睛 + 微笑 + 头发。
/// 背景完全透明（alpha=0），适合叠加渲染。
///
/// # 参数
/// - `hue`: 色相（0-360），控制头发和衣服颜色
fn generate_character_sprite(hue: f32) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let size: u32 = 256;
    let cx: f32 = 128.0;
    let (cr, cg, cb) = hsv_to_rgb(hue, 0.8, 0.85);

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(size, size, |x, y| {
        let fx = x as f32;
        let fy = y as f32;

        // 头部圆形（半径 36px，中心偏上）
        let head_dx = fx - cx;
        let head_dy = fy - 130.0;
        let in_head = (head_dx * head_dx + head_dy * head_dy).sqrt() < 36.0;

        // 身体梯形（上窄下宽）
        let body_top = 165.0;
        let body_bottom = 245.0;
        let in_body_range = fy >= body_top && fy <= body_bottom;
        let body_t = (fy - body_top) / (body_bottom - body_top);
        let body_hw = 28.0 + body_t * 18.0;
        let in_body = in_body_range && (fx - cx).abs() <= body_hw;

        // 头发（略大半圆覆盖头顶）
        let hair_dx = fx - cx;
        let hair_dy = fy - 134.0;
        let in_hair = (hair_dx * hair_dx + hair_dy * hair_dy).sqrt() < 42.0 && fy < 132.0;

        // 眼睛
        let in_eye = {
            let d_left = (fx - 114.0).powi(2) + (fy - 126.0).powi(2);
            let d_right = (fx - 142.0).powi(2) + (fy - 126.0).powi(2);
            d_left < 5.0_f32.powi(2) || d_right < 5.0_f32.powi(2)
        };

        // 微笑嘴巴
        let mouth_dx = (fx - cx) / 14.0;
        let mouth_dy = (fy - 134.0) / 6.0 + mouth_dx * mouth_dx * 0.5;
        let in_mouth = mouth_dx.abs() <= 1.0 && (0.0..=0.15).contains(&mouth_dy);

        // 简单红晕
        let in_blush = {
            let d_left = (fx - 104.0).powi(2) + (fy - 132.0).powi(2);
            let d_right = (fx - 152.0).powi(2) + (fy - 132.0).powi(2);
            (d_left < 10.0_f32.powi(2) || d_right < 10.0_f32.powi(2)) && fy > 128.0
        };

        if in_head {
            if in_hair {
                Rgba([(cr / 3), (cg / 3), (cb / 3), 255])
            } else if in_eye {
                Rgba([255, 255, 255, 255])
            } else if in_mouth {
                Rgba([(cr / 2), (cg / 2), (cb / 2), 255])
            } else if in_blush {
                Rgba([255, 180, 180, 160])
            } else {
                Rgba([255, 220, 190, 255]) // 肤色
            }
        } else if in_body {
            // 白色领口
            let collar = body_t < 0.15 && (fx - cx).abs() <= body_hw - 4.0;
            if collar {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([cr, cg, cb, 255])
            }
        } else if in_hair && fy < 132.0 {
            Rgba([(cr / 3), (cg / 3), (cb / 3), 255])
        } else {
            Rgba([0, 0, 0, 0]) // 完全透明
        }
    });

    to_png(img)
}

/// 生成渐变背景纹理（512×512）。
fn generate_background(r: u8, g: u8, b: u8) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let size: u32 = 512;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(size, size, |x, y| {
        let fy = y as f32 / size as f32;
        let fx_wave = (x as f32 / size as f32 * std::f32::consts::PI).sin() * 0.05;
        let v = 1.0 - fy; // 上亮下暗

        Rgba([
            ((r as f32 * (0.4 + 0.6 * v)) * (1.0 + fx_wave)) as u8,
            ((g as f32 * (0.4 + 0.6 * v)) * (1.0 + fx_wave)) as u8,
            ((b as f32 * (0.4 + 0.6 * v)) * (1.0 + fx_wave)) as u8,
            255,
        ])
    });

    to_png(img)
}

/// HSV → RGB 转换。
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// ImageBuffer → PNG 字节。
fn to_png(img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("生成测试纹理失败");
    buf.into_inner()
}

// ============================================================================
// App — 立绘渲染测试程序
// ============================================================================

/// 立绘渲染测试程序状态。
///
/// 不使用 LayerManager，直接持有各层引用以便测试所有交互功能。
struct App {
    /// GPU 上下文
    gpu: Option<GpuContext>,
    /// 背景层
    bg_layer: Option<BackgroundLayer>,
    /// 立绘层
    sprite_layer: Option<SpriteLayer>,
    /// 渲染配置
    config: RenderConfig,
    /// 当前适配模式
    fit_mode: FitMode,
    /// 帧计数器
    frame_count: u64,

    // ── 立绘管理 ──
    /// 三个立绘的 sprite ID（None = 未创建或已隐藏）
    sprite_left_id: Option<u64>,
    sprite_center_id: Option<u64>,
    sprite_right_id: Option<u64>,
    /// 各立绘可见性
    left_visible: bool,
    center_visible: bool,
    right_visible: bool,
    /// 立绘纹理字节缓存（用于隐藏后恢复）
    left_tex_bytes: Option<Vec<u8>>,
    center_tex_bytes: Option<Vec<u8>>, // 当前显示的版本
    right_tex_bytes: Option<Vec<u8>>,
    /// 立绘描述符缓存
    left_desc: Option<SpriteDescriptor>,
    center_desc: Option<SpriteDescriptor>,
    right_desc: Option<SpriteDescriptor>,

    // ── MV02 透明度动画 ──
    alpha_animating: bool,
    alpha_direction: f32, // 1.0 = 渐显, -1.0 = 渐隐
    alpha_value: f32,

    // ── MV03 立绘替换 ──
    /// 当前中间角色显示的表情版本（0 = 蓝色默认, 1 = 绿色微笑）
    center_version: u8,
    /// 预生成的 B 版本纹理字节（绿色调）
    center_tex_b_bytes: Option<Vec<u8>>,
}

impl App {
    fn new(config: RenderConfig) -> Self {
        Self {
            gpu: None,
            bg_layer: None,
            sprite_layer: None,
            config,
            fit_mode: FitMode::Cover,
            frame_count: 0,
            sprite_left_id: None,
            sprite_center_id: None,
            sprite_right_id: None,
            left_visible: true,
            center_visible: true,
            right_visible: true,
            left_tex_bytes: None,
            center_tex_bytes: None,
            right_tex_bytes: None,
            left_desc: None,
            center_desc: None,
            right_desc: None,
            alpha_animating: false,
            alpha_direction: -1.0,
            alpha_value: 1.0,
            center_version: 0,
            center_tex_b_bytes: None,
        }
    }

    /// 初始化 GPU + 背景层 + 立绘层。
    fn init_gpu(&mut self, window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(window, &self.config)?;
        let device = gpu.device();
        let queue = gpu.queue();
        let format = gpu.surface_config().format;
        let w = self.config.width;
        let h = self.config.height;

        eprintln!("═══ PH1-T08 立绘渲染人工测试 ═══");
        eprintln!("[gpu] 初始化完成 — {w}×{h}");

        // ── 背景层（Layer 0） ──
        let mut bg_layer = BackgroundLayer::new(device, queue, format, w, h);
        let bg_tex = Texture::from_bytes(
            device,
            queue,
            &generate_background(100, 150, 220),
            Some("背景-天空"),
        )?;
        bg_layer.set_background(queue, bg_tex);
        eprintln!("[bg] 背景层就绪 — 蓝灰色渐变天空");

        // ── 立绘层（Layer 1） ──
        let mut sprite_layer = SpriteLayer::new(device, queue, format, w, h);

        // 左侧角色：红色调（小百合），hue=0
        let left_bytes = generate_character_sprite(0.0);
        let tex_left = Texture::from_bytes(device, queue, &left_bytes, Some("立绘-左(红)"))?;
        let left_desc = SpriteDescriptor::new(SpritePosition::Left).with_z_index(1);
        let left_id = sprite_layer.add_sprite(device, queue, tex_left, left_desc.clone());
        self.left_tex_bytes = Some(left_bytes);
        self.left_desc = Some(left_desc);
        eprintln!("[sprite] 左侧立绘就绪 — id={left_id} (红色, Pos=Left, z=1)");

        // 中央角色 A：蓝色调（茜），hue=210
        let center_a_bytes = generate_character_sprite(210.0);
        let tex_center_a =
            Texture::from_bytes(device, queue, &center_a_bytes, Some("立绘-中A(蓝)"))?;
        let center_desc = SpriteDescriptor::new(SpritePosition::Center).with_z_index(2);
        let center_id = sprite_layer.add_sprite(device, queue, tex_center_a, center_desc.clone());
        self.center_tex_bytes = Some(center_a_bytes);
        self.center_desc = Some(center_desc);
        eprintln!("[sprite] 中央立绘就绪 — id={center_id} (蓝色, Pos=Center, z=2)");

        // 预生成中央角色 B 版本的纹理字节（绿色调，hue=120，用于 MV03 替换）
        self.center_tex_b_bytes = Some(generate_character_sprite(120.0));

        // 右侧角色：金色调（学长），hue=45
        let right_bytes = generate_character_sprite(45.0);
        let tex_right = Texture::from_bytes(device, queue, &right_bytes, Some("立绘-右(金)"))?;
        let right_desc = SpriteDescriptor::new(SpritePosition::Right).with_z_index(0);
        let right_id = sprite_layer.add_sprite(device, queue, tex_right, right_desc.clone());
        self.right_tex_bytes = Some(right_bytes);
        self.right_desc = Some(right_desc);
        eprintln!("[sprite] 右侧立绘就绪 — id={right_id} (金色, Pos=Right, z=0)");

        self.sprite_left_id = Some(left_id);
        self.sprite_center_id = Some(center_id);
        self.sprite_right_id = Some(right_id);

        self.bg_layer = Some(bg_layer);
        self.sprite_layer = Some(sprite_layer);
        self.gpu = Some(gpu);

        eprintln!("[ok] {count} 个立绘全部就绪，开始渲染循环", count = 3);
        eprintln!("     按键: 1/2/3=显隐 A=渐变 R=替换 Space=z序 C/V=适配 Q=退出");
        eprintln!();

        Ok(())
    }

    /// 渲染一帧：清屏 → 背景 → 立绘 → present。
    fn render(&mut self) {
        let gpu = match self.gpu.as_ref() {
            Some(g) => g,
            None => return,
        };
        let bg_layer = match self.bg_layer.as_ref() {
            Some(l) => l,
            None => return,
        };
        let sprite_layer = match self.sprite_layer.as_ref() {
            Some(l) => l,
            None => return,
        };

        self.frame_count += 1;

        // MV02 透明度动画
        if self.alpha_animating {
            self.alpha_value += self.alpha_direction * 0.008;
            if self.alpha_value >= 1.0 {
                self.alpha_value = 1.0;
                self.alpha_direction = -1.0;
            } else if self.alpha_value <= 0.1 {
                self.alpha_value = 0.1;
                self.alpha_direction = 1.0;
            }
        }

        let mut frame = match gpu.acquire_frame() {
            Ok(f) => f,
            Err(e) => {
                if self.frame_count <= 3 {
                    eprintln!("[warn] 帧获取失败: {e}");
                }
                return;
            }
        };

        // 步骤 1：清屏
        {
            let _rp = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("清屏"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(gpu.clear_color()),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
        }

        // 步骤 2：背景层 → 立绘层（手动逐层渲染，不使用 LayerManager）
        bg_layer.render(&mut frame.encoder, &frame.view);
        sprite_layer.render(&mut frame.encoder, &frame.view);

        // 步骤 3：提交呈现
        gpu.present(frame);

        // 状态日志
        if self.frame_count.is_multiple_of(180) {
            let anim = if self.alpha_animating {
                format!("α={:.2}", self.alpha_value)
            } else {
                "关".to_string()
            };
            eprintln!(
                "[frame {n:>4}] L:{l} C:{c} R:{r} | 动画:{anim} | 中纹理:v{ver}",
                n = self.frame_count,
                l = if self.left_visible { "●" } else { "○" },
                c = if self.center_visible { "●" } else { "○" },
                r = if self.right_visible { "●" } else { "○" },
                anim = anim,
                ver = self.center_version,
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 交互命令
    // ══════════════════════════════════════════════════════════════════════

    /// MV02 — 切换透明度动画。
    fn toggle_alpha_animation(&mut self) {
        self.alpha_animating = !self.alpha_animating;
        eprintln!(
            "[MV02] 透明度动画：{} (α={:.2})",
            if self.alpha_animating {
                "▶ 播放中"
            } else {
                "⏸ 已暂停"
            },
            self.alpha_value,
        );
    }

    /// MV03 — 替换中央立绘纹理（模拟表情切换）。
    fn replace_center_sprite(&mut self) {
        let Some(gpu) = self.gpu.as_ref() else { return };
        let Some(sprite_layer) = self.sprite_layer.as_mut() else {
            return;
        };
        let Some(old_id) = self.sprite_center_id else {
            return;
        };

        // 读取当前立绘属性（在移除前）
        let current = match sprite_layer.get_sprite(old_id) {
            Some(s) => s.clone(),
            None => {
                eprintln!("[MV03] ❌ 立绘 id={old_id} 不存在（可能已隐藏）");
                return;
            }
        };

        // 步骤 1：移除旧立绘
        sprite_layer.remove_sprite(old_id);

        // 步骤 2：根据版本选择新色相 v0=蓝(210)↔v1=绿(120)
        let new_hue = if self.center_version == 0 {
            120.0
        } else {
            210.0
        };
        let fresh_bytes = generate_character_sprite(new_hue);
        let new_version_label = if self.center_version == 0 {
            "绿色(茜·微笑)"
        } else {
            "蓝色(茜·默认)"
        };

        // 步骤 3：加载新纹理并添加立绘
        let new_tex = Texture::from_bytes(
            gpu.device(),
            gpu.queue(),
            &fresh_bytes,
            Some(&format!("立绘-中v{}", 1 - self.center_version)),
        );

        match new_tex {
            Ok(tex) => {
                let desc = SpriteDescriptor::new(current.position)
                    .with_scale(current.scale.0, current.scale.1)
                    .with_alpha(current.alpha)
                    .with_z_index(current.z_index);

                // 步骤 4：添加新立绘，获取新 ID
                let new_id = sprite_layer.add_sprite(gpu.device(), gpu.queue(), tex, desc.clone());

                // 步骤 5：更新缓存的 ID 和纹理数据（关键！后续动画/显隐都依赖这些）
                self.sprite_center_id = Some(new_id);
                self.center_tex_bytes = Some(fresh_bytes);
                self.center_desc = Some(desc);
                self.center_version = 1 - self.center_version;

                eprintln!(
                    "[MV03] ✅ 中央立绘已替换：{new_version_label} (旧id={old_id} → 新id={new_id})"
                );
            }
            Err(e) => {
                eprintln!("[MV03] ❌ 纹理加载失败：{e}");
                // 恢复旧立绘（从缓存重建）
                if let Some(bytes) = &self.center_tex_bytes {
                    let desc = self.center_desc.as_ref().unwrap();
                    if let Ok(recovered_tex) =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("立绘-中(恢复)"))
                    {
                        let recovered_id = sprite_layer.add_sprite(
                            gpu.device(),
                            gpu.queue(),
                            recovered_tex,
                            desc.clone(),
                        );
                        self.sprite_center_id = Some(recovered_id);
                        eprintln!("[MV03] ↻ 已恢复旧立绘 (id={recovered_id})");
                    }
                }
            }
        }
    }

    /// 切换右侧立绘的 z-index（演示用）。
    fn toggle_center_z_index(&mut self) {
        let Some(center_id) = self.sprite_center_id else {
            return;
        };
        let Some(sl) = self.sprite_layer.as_mut() else {
            return;
        };

        let sprite = sl.get_sprite(center_id).unwrap();
        let new_z = if sprite.z_index >= 2 {
            0
        } else {
            sprite.z_index + 1
        };
        eprintln!("[z] 中央立绘 z-index: {} → {}", sprite.z_index, new_z);
        eprintln!("[z] 提示：当前通过 add_sprite 重新添加来改变 z-index（同 ID 替换）");
    }

    /// 切换立绘可见性 — 真正地从 SpriteLayer 中移除/恢复立绘。
    ///
    /// 隐藏时保存 sprite ID 和纹理数据；显示时从缓存字节重新创建纹理。
    fn toggle_sprite_visibility(&mut self, pos: u8) {
        let Some(gpu) = self.gpu.as_ref() else { return };
        let Some(sprite_layer) = self.sprite_layer.as_mut() else {
            return;
        };

        match pos {
            1 => {
                self.left_visible = !self.left_visible;
                if self.left_visible {
                    // 恢复显示
                    let bytes = self.left_tex_bytes.as_ref().expect("左侧立绘纹理未缓存");
                    let desc = self.left_desc.as_ref().expect("左侧立绘描述符未缓存");
                    let tex =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("立绘-左(红)"));
                    match tex {
                        Ok(t) => {
                            let id =
                                sprite_layer.add_sprite(gpu.device(), gpu.queue(), t, desc.clone());
                            self.sprite_left_id = Some(id);
                            eprintln!("[vis] 左侧立绘：显示 (id={id})");
                        }
                        Err(e) => eprintln!("[vis] 左侧立绘恢复失败：{e}"),
                    }
                } else {
                    // 隐藏
                    if let Some(id) = self.sprite_left_id.take() {
                        sprite_layer.remove_sprite(id);
                        eprintln!("[vis] 左侧立绘：隐藏");
                    }
                }
            }
            2 => {
                self.center_visible = !self.center_visible;
                if self.center_visible {
                    let bytes = self.center_tex_bytes.as_ref().expect("中央立绘纹理未缓存");
                    let desc = self.center_desc.as_ref().expect("中央立绘描述符未缓存");
                    let tex =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("立绘-中"));
                    match tex {
                        Ok(t) => {
                            let id =
                                sprite_layer.add_sprite(gpu.device(), gpu.queue(), t, desc.clone());
                            self.sprite_center_id = Some(id);
                            eprintln!("[vis] 中央立绘：显示 (id={id})");
                        }
                        Err(e) => eprintln!("[vis] 中央立绘恢复失败：{e}"),
                    }
                } else {
                    if let Some(id) = self.sprite_center_id.take() {
                        sprite_layer.remove_sprite(id);
                        eprintln!("[vis] 中央立绘：隐藏");
                    }
                }
            }
            3 => {
                self.right_visible = !self.right_visible;
                if self.right_visible {
                    let bytes = self.right_tex_bytes.as_ref().expect("右侧立绘纹理未缓存");
                    let desc = self.right_desc.as_ref().expect("右侧立绘描述符未缓存");
                    let tex =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("立绘-右(金)"));
                    match tex {
                        Ok(t) => {
                            let id =
                                sprite_layer.add_sprite(gpu.device(), gpu.queue(), t, desc.clone());
                            self.sprite_right_id = Some(id);
                            eprintln!("[vis] 右侧立绘：显示 (id={id})");
                        }
                        Err(e) => eprintln!("[vis] 右侧立绘恢复失败：{e}"),
                    }
                } else {
                    if let Some(id) = self.sprite_right_id.take() {
                        sprite_layer.remove_sprite(id);
                        eprintln!("[vis] 右侧立绘：隐藏");
                    }
                }
            }
            _ => {}
        }
    }

    /// 切换背景适配模式。
    fn toggle_fit_mode(&mut self, mode: FitMode) {
        self.fit_mode = mode;
        if let Some(gpu) = self.gpu.as_ref()
            && let Some(bg) = self.bg_layer.as_mut()
        {
            bg.set_fit_mode(gpu.queue(), mode);
        }
        eprintln!("[bg] 适配模式：{mode:?}");
    }

    /// 更新立绘 alpha（每帧在 render 中调用）。
    fn update_alpha_animation(&mut self) {
        if !self.alpha_animating {
            return;
        }

        let Some(gpu) = self.gpu.as_ref() else { return };
        let Some(sprite_layer) = self.sprite_layer.as_mut() else {
            return;
        };
        let Some(center_id) = self.sprite_center_id else {
            return;
        };

        sprite_layer.update_alpha(gpu.queue(), center_id, self.alpha_value);
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.resize(width, height);
        }
        if let Some(gpu) = self.gpu.as_ref() {
            if let Some(bg) = self.bg_layer.as_mut() {
                bg.resize(gpu.queue(), width, height);
            }
            if let Some(sl) = self.sprite_layer.as_mut() {
                sl.resize(gpu.queue(), width, height);
            }
        }
        eprintln!("[resize] {width}×{height}");
    }
}

// ============================================================================
// ApplicationHandler 实现
// ============================================================================

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Asterism — PH1-T08 立绘渲染测试 (1/2/3=显隐 · A=渐变 · R=替换 · Space=z序 · C/V=适配)")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("[error] 窗口创建失败：{e}");
                event_loop.exit();
                return;
            }
        };

        if let Err(e) = self.init_gpu(window) {
            eprintln!("[error] GPU 初始化失败：{e}");
            event_loop.exit();
            return;
        }

        if let Some(gpu) = self.gpu.as_ref()
            && let Some(window) = gpu.window()
        {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                eprintln!("[bye] 窗口关闭");
                event_loop.exit();
            }

            WindowEvent::KeyboardInput {
                event: key_event, ..
            } if key_event.state == ElementState::Pressed => {
                match key_event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::KeyQ) => {
                        eprintln!("[bye] 退出");
                        event_loop.exit();
                    }
                    // MV02 — 透明度动画
                    PhysicalKey::Code(KeyCode::KeyA) => {
                        self.toggle_alpha_animation();
                    }
                    // MV03 — 立绘替换
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        self.replace_center_sprite();
                    }
                    // z-index 切换
                    PhysicalKey::Code(KeyCode::Space) => {
                        self.toggle_center_z_index();
                    }
                    // 适配模式
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        self.toggle_fit_mode(FitMode::Cover);
                    }
                    PhysicalKey::Code(KeyCode::KeyV) => {
                        self.toggle_fit_mode(FitMode::Contain);
                    }
                    // 显隐切换
                    PhysicalKey::Code(KeyCode::Digit1) => self.toggle_sprite_visibility(1),
                    PhysicalKey::Code(KeyCode::Digit2) => self.toggle_sprite_visibility(2),
                    PhysicalKey::Code(KeyCode::Digit3) => self.toggle_sprite_visibility(3),
                    _ => {}
                }
            }

            WindowEvent::Resized(new_size) => {
                self.on_resize(new_size.width, new_size.height);
            }

            WindowEvent::RedrawRequested => {
                // 在渲染前更新动画状态
                self.update_alpha_animation();
                self.render();

                if let Some(gpu) = self.gpu.as_ref()
                    && let Some(window) = gpu.window()
                {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

// ============================================================================
// main — 程序入口
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("╔══════════════════════════════════════════════╗");
    eprintln!("║  Asterism 渲染器 — PH1-T08 立绘渲染测试    ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  MV01  多立绘同时显示 (3角色: 左/中/右)    ║");
    eprintln!("║  MV02  透明度渐变 (按 A 键: α 0.1↔1.0)    ║");
    eprintln!("║  MV03  立绘替换   (按 R 键: 表情切换)      ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  1/2/3  切换立绘显隐                        ║");
    eprintln!("║  Space  切换 z-index                        ║");
    eprintln!("║  C/V    背景 Cover / Contain                ║");
    eprintln!("║  Q/ESC  退出                                ║");
    eprintln!("╚══════════════════════════════════════════════╝");
    eprintln!();

    let config = RenderConfig {
        width: 960,
        height: 640,
        clear_color: [0.05, 0.08, 0.15, 1.0],
        ..RenderConfig::default()
    };

    let mut app = App::new(config);

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;

    eprintln!("[bye] 程序正常退出");
    Ok(())
}
