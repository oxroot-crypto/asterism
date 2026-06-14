//! Asterism — Galgame/ADV 游戏引擎
//!
//! 示例：集成渲染测试（背景 + 立绘 + 文本）
//!
//! 功能概述：
//!   - 覆盖 PH1-T07（背景层）、PH1-T08（立绘层）、PH1-T09（文本渲染）
//!   - 程序化生成 3 个角色立绘 + 渐变背景 + 多段对话文本
//!   - 使用 LayerManager 管理图层栈（Layer 0 背景 / Layer 1 立绘 / Layer 4 文本）
//!   - 模拟视觉小说的基本演出流程：背景 → 角色登场 → 对话推进
//!
//! 运行方式：
//!   cargo run --package aster-renderer --example integration_demo
//!
//! 交互：
//!   - N / Enter / Space  推进对话（下一段文本）
//!   - 1 / 2 / 3          切换角色立绘显隐
//!   - A                  切换透明度动画
//!   - R                  替换中央角色表情
//!   - T                  切换文本显隐
//!   - C / V              切换背景 Cover / Contain
//!   - ESC / Q            退出
//!
//! 作者：Claude (AI)
//! 创建日期：2026-06-14

use std::sync::Arc;

use aster_renderer::{
    BackgroundLayer, FitMode, GpuContext, Layer, RenderConfig, SpriteDescriptor, SpriteLayer,
    SpritePosition, TextConfig, TextRenderer, Texture,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

// ============================================================================
// 对话脚本
// ============================================================================

/// 对话条目 — 预定义的演示对话序列。
const DIALOGUES: &[(&str, &str)] = &[
    ("", "夕阳把教室染成一片温暖的橙色。"),
    ("", "放学后的教室里，只剩下她一个人。"),
    ("小百合", "……你来了啊。"),
    ("小百合", "我还以为你今天不会来了呢。"),
    ("", "她转过身，逆光的轮廓有些模糊。"),
    ("小百合", "其实……我有话想对你说。"),
    ("", "窗外的风吹动了窗帘。"),
    ("小百合", "谢谢你一直陪在我身边。"),
    ("小百合", "从今往后……也请多指教了。"),
    ("", "——那一刻，我听到了自己心跳的声音。"),
];

// ============================================================================
// 程序化纹理生成
// ============================================================================

/// 生成渐变背景纹理（模拟夕阳教室）。
fn generate_classroom_background() -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let size: u32 = 512;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(size, size, |x, y| {
        let fx = x as f32 / size as f32;
        let fy = y as f32 / size as f32;

        // 天空渐变：上橙下暗（模拟黄昏教室）
        let sky_r = 255.0 * (0.9 - fy * 0.5);
        let sky_g = 160.0 * (0.7 - fy * 0.5);
        let sky_b = 80.0 * (0.5 - fy * 0.4);

        // 窗户光柱
        let window_light = {
            let wx = (fx * 5.0).fract();
            let dist = (wx - 0.5).abs();
            if dist < 0.15 {
                0.3 + 0.2 * (1.0 - dist / 0.15)
            } else {
                0.0
            }
        };

        // 地板反光
        let floor = if fy > 0.7 { 0.1 * (1.0 - fy) } else { 0.0 };

        let brightness = 1.0 + window_light + floor;

        Rgba([
            (sky_r * brightness).min(255.0) as u8,
            (sky_g * brightness).min(255.0) as u8,
            (sky_b * brightness).min(255.0) as u8,
            255,
        ])
    });

    to_png(img)
}

/// 生成带 Alpha 通道的角色立绘纹理（256×256）。
///
/// 绘制简笔画风格角色：头部 + 身体 + 眼睛 + 头发。
fn generate_character_sprite(hue: f32) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let size: u32 = 256;
    let cx: f32 = 128.0;
    let (cr, cg, cb) = hsv_to_rgb(hue, 0.8, 0.85);

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(size, size, |x, y| {
        let fx = x as f32;
        let fy = y as f32;

        // 头部圆形
        let head_dx = fx - cx;
        let head_dy = fy - 130.0;
        let in_head = (head_dx * head_dx + head_dy * head_dy).sqrt() < 36.0;

        // 身体梯形
        let body_top = 165.0;
        let body_bottom = 245.0;
        let in_body_range = fy >= body_top && fy <= body_bottom;
        let body_t = (fy - body_top) / (body_bottom - body_top);
        let body_hw = 28.0 + body_t * 18.0;
        let in_body = in_body_range && (fx - cx).abs() <= body_hw;

        // 头发
        let hair_dx = fx - cx;
        let hair_dy = fy - 134.0;
        let in_hair = (hair_dx * hair_dx + hair_dy * hair_dy).sqrt() < 42.0 && fy < 132.0;

        // 眼睛
        let in_eye = {
            let d_left = (fx - 114.0).powi(2) + (fy - 126.0).powi(2);
            let d_right = (fx - 142.0).powi(2) + (fy - 126.0).powi(2);
            d_left < 5.0_f32.powi(2) || d_right < 5.0_f32.powi(2)
        };

        // 微笑
        let mx = (fx - cx) / 14.0;
        let my = (fy - 134.0) / 6.0 + mx * mx * 0.5;
        let in_mouth = mx.abs() <= 1.0 && (0.0..=0.15).contains(&my);

        // 红晕
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
            let collar = body_t < 0.15 && (fx - cx).abs() <= body_hw - 4.0;
            if collar {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([cr, cg, cb, 255])
            }
        } else if in_hair && fy < 132.0 {
            Rgba([(cr / 3), (cg / 3), (cb / 3), 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    to_png(img)
}

/// HSV → RGB。
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

/// 安全截断 UTF-8 字符串到指定字节数（不超过 char 边界）。
fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// ImageBuffer → PNG 字节。
fn to_png(img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("生成测试纹理失败");
    buf.into_inner()
}

// ============================================================================
// App — 集成渲染测试程序
// ============================================================================

/// 集成演示状态 — 模拟一个简单的视觉小说场景。
struct App {
    /// GPU 上下文
    gpu: Option<GpuContext>,
    /// 背景层
    bg_layer: Option<BackgroundLayer>,
    /// 立绘层
    sprite_layer: Option<SpriteLayer>,
    /// 文本渲染器
    text_renderer: Option<TextRenderer>,
    /// 渲染配置
    config: RenderConfig,
    /// 文本配置
    text_config: TextConfig,
    /// 帧计数器
    frame_count: u64,

    // ── 立绘管理 ──
    sprite_left_id: Option<u64>,
    sprite_center_id: Option<u64>,
    sprite_right_id: Option<u64>,
    left_visible: bool,
    center_visible: bool,
    right_visible: bool,
    left_tex_bytes: Option<Vec<u8>>,
    center_tex_bytes: Option<Vec<u8>>,
    right_tex_bytes: Option<Vec<u8>>,
    left_desc: Option<SpriteDescriptor>,
    center_desc: Option<SpriteDescriptor>,
    right_desc: Option<SpriteDescriptor>,

    // ── 对话管理 ──
    dialogue_index: usize,
    text_visible: bool,

    // ── 动画 ──
    alpha_animating: bool,
    alpha_direction: f32,
    alpha_value: f32,
    center_version: u8,
}

impl App {
    fn new(config: RenderConfig, text_config: TextConfig) -> Self {
        Self {
            gpu: None,
            bg_layer: None,
            sprite_layer: None,
            text_renderer: None,
            config,
            text_config,
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
            dialogue_index: 0,
            text_visible: true,
            alpha_animating: false,
            alpha_direction: -1.0,
            alpha_value: 1.0,
            center_version: 0,
        }
    }

    /// 初始化 GPU + 背景 + 立绘 + 文本渲染器。
    fn init_gpu(&mut self, window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(window, &self.config)?;
        let device = gpu.device();
        let queue = gpu.queue();
        let format = gpu.surface_config().format;
        let w = self.config.width;
        let h = self.config.height;

        eprintln!("╔══════════════════════════════════════════╗");
        eprintln!("║   Asterism 集成渲染测试                  ║");
        eprintln!("║   PH1-T07 背景 + T08 立绘 + T09 文本    ║");
        eprintln!("╚══════════════════════════════════════════╝");
        eprintln!();

        // ── 背景层 (Layer 0) ──
        eprintln!("[T07] 初始化背景层...");
        let bg_tex = Texture::from_bytes(
            device,
            queue,
            &generate_classroom_background(),
            Some("bg:黄昏教室"),
        )?;
        let mut bg_layer = BackgroundLayer::new(device, queue, format, w, h);
        bg_layer.set_background(queue, bg_tex);
        eprintln!("  ✓ 背景层就绪 — 黄昏教室");

        // ── 立绘层 (Layer 1) ──
        eprintln!("[T08] 初始化立绘层...");
        let mut sprite_layer = SpriteLayer::new(device, queue, format, w, h);

        // 左侧：小百合 (红色调, hue=0)
        let left_bytes = generate_character_sprite(0.0);
        let tex_left = Texture::from_bytes(device, queue, &left_bytes, Some("sprite:小百合"))?;
        let left_desc = SpriteDescriptor::new(SpritePosition::Left).with_z_index(1);
        let left_id = sprite_layer.add_sprite(device, queue, tex_left, left_desc.clone());
        self.left_tex_bytes = Some(left_bytes);
        self.left_desc = Some(left_desc);
        self.sprite_left_id = Some(left_id);
        eprintln!("  ✓ 左侧立绘 — 小百合 (红色, z=1)");

        // 中央：茜 (蓝色调, hue=210)
        let center_bytes = generate_character_sprite(210.0);
        let tex_center = Texture::from_bytes(device, queue, &center_bytes, Some("sprite:茜"))?;
        let center_desc = SpriteDescriptor::new(SpritePosition::Center).with_z_index(2);
        let center_id = sprite_layer.add_sprite(device, queue, tex_center, center_desc.clone());
        self.center_tex_bytes = Some(center_bytes);
        self.center_desc = Some(center_desc);
        self.sprite_center_id = Some(center_id);
        eprintln!("  ✓ 中央立绘 — 茜 (蓝色, z=2)");

        // 右侧：学长 (金色调, hue=45)
        let right_bytes = generate_character_sprite(45.0);
        let tex_right = Texture::from_bytes(device, queue, &right_bytes, Some("sprite:学长"))?;
        let right_desc = SpriteDescriptor::new(SpritePosition::Right).with_z_index(0);
        let right_id = sprite_layer.add_sprite(device, queue, tex_right, right_desc.clone());
        self.right_tex_bytes = Some(right_bytes);
        self.right_desc = Some(right_desc);
        self.sprite_right_id = Some(right_id);
        eprintln!("  ✓ 右侧立绘 — 学长 (金色, z=0)");

        // ── 文本渲染器 (Layer 4) ──
        eprintln!("[T09] 初始化文本渲染器...");
        let mut text_renderer =
            TextRenderer::new(device, queue, format, w, h, self.text_config.clone())?;
        // 设置初始对话
        let (speaker, body) = DIALOGUES[self.dialogue_index];
        text_renderer.set_text(speaker, body);
        text_renderer.prepare(device, queue);
        eprintln!(
            "  ✓ 文本渲染器就绪 — 对话 #{idx}: 「{speaker}」\"{body}\"",
            idx = self.dialogue_index,
            speaker = if speaker.is_empty() {
                "旁白"
            } else {
                speaker
            },
            body = truncate_str(body, 20),
        );

        self.bg_layer = Some(bg_layer);
        self.sprite_layer = Some(sprite_layer);
        self.text_renderer = Some(text_renderer);
        self.gpu = Some(gpu);

        eprintln!();
        eprintln!(
            "  按键: N/Enter=下一句 · 1/2/3=显隐 · A=渐变 · R=替换表情 · T=隐藏文本 · Q=退出"
        );
        eprintln!();

        Ok(())
    }

    /// 渲染一帧：清屏 → 背景 → 立绘 → 文本 → present。
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

        // 透明度动画
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

        // 步骤 2：Layer 0 — 背景层
        bg_layer.render(&mut frame.encoder, &frame.view);

        // 步骤 3：Layer 1 — 立绘层
        sprite_layer.render(&mut frame.encoder, &frame.view);

        // 步骤 4：Layer 4 — 文本层
        if self.text_visible
            && let Some(ref renderer) = self.text_renderer
        {
            renderer.render(&mut frame.encoder, &frame.view);
        }

        // 步骤 5：提交呈现
        gpu.present(frame);

        // 帧状态日志
        if self.frame_count.is_multiple_of(180) {
            let (speaker, _body) = DIALOGUES[self.dialogue_index];
            eprintln!(
                "[frame {n:>4}] 对话 #{idx}: {spk} | L:{l} C:{c} R:{r} | txt:{t} anim:{anim}",
                n = self.frame_count,
                idx = self.dialogue_index,
                spk = if speaker.is_empty() {
                    "旁白"
                } else {
                    speaker
                },
                l = if self.left_visible { "●" } else { "○" },
                c = if self.center_visible { "●" } else { "○" },
                r = if self.right_visible { "●" } else { "○" },
                t = if self.text_visible { "●" } else { "○" },
                anim = if self.alpha_animating {
                    format!("α={:.2}", self.alpha_value)
                } else {
                    "—".into()
                },
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 交互命令
    // ══════════════════════════════════════════════════════════════════════

    /// 推进到下一段对话。
    fn advance_dialogue(&mut self) {
        self.dialogue_index = (self.dialogue_index + 1) % DIALOGUES.len();
        let (speaker, body) = DIALOGUES[self.dialogue_index];
        eprintln!(
            "[dialogue #{idx}] {spk}: \"{body}\"",
            idx = self.dialogue_index,
            spk = if speaker.is_empty() {
                "旁白"
            } else {
                speaker
            },
            body = body,
        );

        if let Some(ref gpu) = self.gpu
            && let Some(ref mut renderer) = self.text_renderer
        {
            renderer.set_text(speaker, body);
            renderer.prepare(gpu.device(), gpu.queue());
        }
    }

    /// 切换文本显隐。
    fn toggle_text_visibility(&mut self) {
        self.text_visible = !self.text_visible;
        eprintln!(
            "[text] 文本显隐：{}",
            if self.text_visible {
                "显示"
            } else {
                "隐藏"
            }
        );
    }

    /// 切换透明度动画。
    fn toggle_alpha_animation(&mut self) {
        self.alpha_animating = !self.alpha_animating;
        eprintln!(
            "[alpha] 透明度动画：{}",
            if self.alpha_animating {
                "▶ 播放中"
            } else {
                "⏸ 已暂停"
            },
        );
    }

    /// 替换中央立绘纹理（模拟表情切换）。
    fn replace_center_sprite(&mut self) {
        let Some(gpu) = self.gpu.as_ref() else { return };
        let Some(sprite_layer) = self.sprite_layer.as_mut() else {
            return;
        };
        let Some(old_id) = self.sprite_center_id else {
            return;
        };

        let current = match sprite_layer.get_sprite(old_id) {
            Some(s) => s.clone(),
            None => {
                eprintln!("[replace] 立绘不存在");
                return;
            }
        };

        // 移除旧立绘
        sprite_layer.remove_sprite(old_id);

        // 生成新表情 (v0=蓝→绿, v1=绿→蓝)
        let new_hue = if self.center_version == 0 {
            120.0
        } else {
            210.0
        };
        let new_bytes = generate_character_sprite(new_hue);
        let label = if self.center_version == 0 {
            "茜·微笑(绿)"
        } else {
            "茜·默认(蓝)"
        };

        match Texture::from_bytes(gpu.device(), gpu.queue(), &new_bytes, Some(label)) {
            Ok(tex) => {
                let desc = SpriteDescriptor::new(current.position)
                    .with_scale(current.scale.0, current.scale.1)
                    .with_alpha(current.alpha)
                    .with_z_index(current.z_index);
                let new_id = sprite_layer.add_sprite(gpu.device(), gpu.queue(), tex, desc.clone());
                self.sprite_center_id = Some(new_id);
                self.center_tex_bytes = Some(new_bytes);
                self.center_desc = Some(desc);
                self.center_version = 1 - self.center_version;
                eprintln!("[replace] ✅ {label} (id={new_id})");
            }
            Err(e) => eprintln!("[replace] ❌ {e}"),
        }
    }

    /// 切换立绘显隐。
    fn toggle_sprite_visibility(&mut self, pos: u8) {
        let Some(gpu) = self.gpu.as_ref() else { return };
        let Some(sprite_layer) = self.sprite_layer.as_mut() else {
            return;
        };

        match pos {
            1 => {
                self.left_visible = !self.left_visible;
                if self.left_visible {
                    let bytes = self.left_tex_bytes.as_ref().unwrap();
                    let desc = self.left_desc.as_ref().unwrap();
                    if let Ok(tex) =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("sprite:小百合"))
                    {
                        let id =
                            sprite_layer.add_sprite(gpu.device(), gpu.queue(), tex, desc.clone());
                        self.sprite_left_id = Some(id);
                    }
                } else if let Some(id) = self.sprite_left_id.take() {
                    sprite_layer.remove_sprite(id);
                }
                eprintln!(
                    "[vis] 左侧小百合：{}",
                    if self.left_visible { "●" } else { "○" }
                );
            }
            2 => {
                self.center_visible = !self.center_visible;
                if self.center_visible {
                    let bytes = self.center_tex_bytes.as_ref().unwrap();
                    let desc = self.center_desc.as_ref().unwrap();
                    if let Ok(tex) =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("sprite:茜"))
                    {
                        let id =
                            sprite_layer.add_sprite(gpu.device(), gpu.queue(), tex, desc.clone());
                        self.sprite_center_id = Some(id);
                    }
                } else if let Some(id) = self.sprite_center_id.take() {
                    sprite_layer.remove_sprite(id);
                }
                eprintln!(
                    "[vis] 中央茜：{}",
                    if self.center_visible { "●" } else { "○" }
                );
            }
            3 => {
                self.right_visible = !self.right_visible;
                if self.right_visible {
                    let bytes = self.right_tex_bytes.as_ref().unwrap();
                    let desc = self.right_desc.as_ref().unwrap();
                    if let Ok(tex) =
                        Texture::from_bytes(gpu.device(), gpu.queue(), bytes, Some("sprite:学长"))
                    {
                        let id =
                            sprite_layer.add_sprite(gpu.device(), gpu.queue(), tex, desc.clone());
                        self.sprite_right_id = Some(id);
                    }
                } else if let Some(id) = self.sprite_right_id.take() {
                    sprite_layer.remove_sprite(id);
                }
                eprintln!(
                    "[vis] 右侧学长：{}",
                    if self.right_visible { "●" } else { "○" }
                );
            }
            _ => {}
        }
    }

    /// 切换背景适配模式。
    fn toggle_fit_mode(&mut self, mode: FitMode) {
        if let Some(gpu) = self.gpu.as_ref()
            && let Some(bg) = self.bg_layer.as_mut()
        {
            bg.set_fit_mode(gpu.queue(), mode);
        }
        eprintln!("[fit] {mode:?}");
    }

    /// 更新立绘 alpha 动画（在 render 前调用）。
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

    /// 响应窗口 resize。
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
            if let Some(tr) = self.text_renderer.as_mut() {
                tr.resize(width, height);
                tr.prepare(gpu.device(), gpu.queue());
            }
        }
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

        let title =
            "Asterism — 集成渲染测试 (T07背景+T08立绘+T09文本) | N=下一句 Q=退出".to_string();
        let window_attrs = Window::default_attributes()
            .with_title(title)
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
                    // 推进对话
                    PhysicalKey::Code(KeyCode::KeyN)
                    | PhysicalKey::Code(KeyCode::Enter)
                    | PhysicalKey::Code(KeyCode::Space) => {
                        self.advance_dialogue();
                    }
                    // 文本显隐
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        self.toggle_text_visibility();
                    }
                    // 透明度动画
                    PhysicalKey::Code(KeyCode::KeyA) => {
                        self.toggle_alpha_animation();
                    }
                    // 替换表情
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        self.replace_center_sprite();
                    }
                    // 适配模式
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        self.toggle_fit_mode(FitMode::Cover);
                    }
                    PhysicalKey::Code(KeyCode::KeyV) => {
                        self.toggle_fit_mode(FitMode::Contain);
                    }
                    // 显隐
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
    eprintln!("║   Asterism 集成渲染测试                      ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║   PH1-T07  背景图层渲染    ✓               ║");
    eprintln!("║   PH1-T08  角色立绘渲染    ✓               ║");
    eprintln!("║   PH1-T09  文本渲染        ✓               ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  场景：黄昏教室，小百合在等你                ║");
    eprintln!("║  角色：小百合(左) · 茜(中) · 学长(右)        ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  N / Enter / Space  推进对话                ║");
    eprintln!("║  T                  切换文本显隐            ║");
    eprintln!("║  1 / 2 / 3          切换角色显隐            ║");
    eprintln!("║  A                  透明度动画              ║");
    eprintln!("║  R                  替换表情                ║");
    eprintln!("║  C / V              背景 Cover/Contain      ║");
    eprintln!("║  Q / ESC            退出                    ║");
    eprintln!("╚══════════════════════════════════════════════╝");
    eprintln!();

    let config = RenderConfig {
        width: 960,
        height: 640,
        clear_color: [0.05, 0.08, 0.15, 1.0],
        ..RenderConfig::default()
    };

    // 文本配色：白色正文 + 浅金色说话者名字
    let text_config = TextConfig {
        font_size: 26.0,
        speaker_font_size: 22.0,
        line_height: 1.6,
        text_color: [1.0, 1.0, 1.0, 1.0],
        speaker_color: [1.0, 0.85, 0.6, 1.0], // 暖金色，与黄昏场景协调
        text_box_padding: 24.0,
    };

    let mut app = App::new(config, text_config);

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;

    eprintln!("[bye] 程序正常退出");
    Ok(())
}
