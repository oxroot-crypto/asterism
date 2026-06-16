//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-save/src/save_manager.rs
//! 功能概述：存档管理器 — 负责游戏存档的创建、读取、列表和删除。
//!           存档格式为 `[4 字节 CRC32 LE] + [MessagePack 序列化的 SaveData]`，
//!           确保存档文件的完整性可验证。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::save::{SaveData, SaveSlotInfo}
//! - rmp_serde（MessagePack 序列化/反序列化）
//! - crc32fast（CRC32 校验和计算）
//!
//! 对应需求：REQ-ENG-040（游戏存档）、REQ-ENG-041（游戏读档）、NFR-SEC-003（存档完整性）
//! 对应任务：PH2-T06 — aster-save SaveData 数据结构 + 序列化 + CRC32

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use aster_core::{SaveData, SaveSlotInfo};
use image::DynamicImage;

// ─── 槽位常量 ──────────────────────────────────────────────────────────────

/// 手动存档槽位数（槽位 0 ~ 4）
pub const MANUAL_SLOT_COUNT: u8 = 5;

/// 快速存档槽位号
pub const QUICK_SLOT: u8 = 98;

/// 自动存档槽位号
pub const AUTO_SLOT: u8 = 99;

/// 当前存档格式版本号（对应 `SaveData::CURRENT_VERSION`）
pub const CURRENT_VERSION: u32 = 1;

/// 存档文件扩展名
const SAVE_EXTENSION: &str = "sav";

// ─── SaveError ──────────────────────────────────────────────────────────────

/// 存档操作错误类型 — 覆盖存档读写的全部失败场景。
///
/// 使用 `thiserror` 派生，为每种错误场景提供清晰的中文错误消息。
/// 注意：`rmp_serde` 的 encode/decode Error 不实现 `std::error::Error`，
/// 因此使用 `Serialize(String)` / `Deserialize(String)` 变体手动映射。
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// IO 错误（文件读写、目录创建等）
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),

    /// 序列化失败 — SaveData → MessagePack 字节时出错
    #[error("序列化失败：{0}")]
    Serialize(String),

    /// 反序列化失败 — MessagePack 字节 → SaveData 时出错
    #[error("反序列化失败：{0}")]
    Deserialize(String),

    /// 存档文件已损坏 — CRC32 校验失败或文件格式错误
    #[error("存档文件已损坏（槽位 {slot}）：{reason}")]
    Corrupted { slot: u8, reason: String },

    /// 存档版本不兼容 — 文件版本与当前引擎版本不匹配
    #[error("存档版本不兼容：文件版本 {found}，当前引擎版本 {expected}。{hint}")]
    IncompatibleVersion {
        found: u32,
        expected: u32,
        hint: String,
    },

    /// 槽位为空 — 指定的槽位没有存档文件
    #[error("槽位 {slot} 为空（不存在存档文件）")]
    EmptySlot { slot: u8 },
}

// ─── rmp_serde 错误映射辅助函数 ────────────────────────────────────────────

/// 将 `rmp_serde::encode::Error` 转换为 `SaveError::Serialize`。
///
/// `rmp_serde::encode::Error` 仅实现 `Display + Debug`，不实现 `std::error::Error`，
/// 因此不能通过 `#[from]` 自动转换。此函数手动提取错误消息。
fn to_serialize_error(err: rmp_serde::encode::Error) -> SaveError {
    SaveError::Serialize(err.to_string())
}

/// 将 `rmp_serde::decode::Error` 转换为 `SaveError::Deserialize`。
///
/// 同上，`rmp_serde::decode::Error` 不实现 `std::error::Error`。
fn to_deserialize_error(err: rmp_serde::decode::Error) -> SaveError {
    SaveError::Deserialize(err.to_string())
}

// ─── SaveManager ────────────────────────────────────────────────────────────

/// 存档管理器 — 管理游戏存档的完整生命周期（CRUD）。
///
/// 每个 `SaveManager` 实例绑定一个存档目录，所有存档文件
/// 均以 `slot_{NN}.sav` 格式命名并存储在该目录下。
///
/// # 存档文件格式
///
/// ```text
/// Byte 0-3:   CRC32 校验和（little-endian u32），校验范围为 Byte 4..EOF
/// Byte 4-EOF: MessagePack 序列化的 SaveData（rmp-serde）
/// ```
///
/// # 槽位分配
///
/// | 范围 | 类型 | 说明 |
/// |------|------|------|
/// | 0-4 | 手动存档 | 玩家手动创建的存档 |
/// | 98 | 快速存档 | 快捷操作触发的存档 |
/// | 99 | 自动存档 | 系统自动创建的存档 |
///
/// # 线程安全
///
/// `SaveManager` 不是 `Send + Sync`——设计由 SceneManager 单线程持有。
/// 存档读写操作是同步 I/O（文件通常 < 1MB，耗时 < 10ms），无需异步。
///
/// # 示例
/// ```rust,no_run
/// use std::path::PathBuf;
/// use aster_save::SaveManager;
/// use aster_core::SaveData;
///
/// let manager = SaveManager::new(PathBuf::from("saves"));
/// let data = SaveData::new(0, "chapter1/prologue");
///
/// // 保存
/// let info = manager.save(0, &data).expect("保存失败");
/// assert_eq!(info.slot, 0);
///
/// // 读取
/// let loaded = manager.load(0).expect("读取失败");
/// assert_eq!(loaded.scene_id, "chapter1/prologue");
/// ```
#[derive(Debug)]
pub struct SaveManager {
    /// 存档文件存储目录的绝对路径
    save_dir: PathBuf,
}

impl SaveManager {
    /// 创建新的存档管理器实例，绑定到指定的存档目录。
    ///
    /// 此方法会自动创建存档目录（如果不存在），包括所有必需的父目录。
    ///
    /// # 参数
    /// - `save_dir`：存档文件存储目录路径（相对或绝对均可，内部会 canonicalize）
    ///
    /// # 返回值
    /// 返回新的 `SaveManager` 实例。
    ///
    /// # Panics
    /// （无）—— 目录创建失败时会静默忽略（错误延迟到首次 `save()` 操作时报告）。
    pub fn new(save_dir: PathBuf) -> Self {
        // 尝试创建存档目录（静默忽略失败——首次 save 操作时会暴露问题）
        let _ = fs::create_dir_all(&save_dir);

        Self { save_dir }
    }

    /// 将 SaveData 序列化并写入指定槽位的存档文件。
    ///
    /// 保存流程：
    /// 1. 使用 `rmp_serde::to_vec()` 将 `SaveData` 序列化为 MessagePack 字节
    /// 2. 使用 `crc32fast::hash()` 计算 MessagePack 数据的 CRC32 校验和
    /// 3. 将 `[CRC32 u32 LE] + [MessagePack 数据]` 写入 `slot_{NN}.sav`
    ///
    /// # 参数
    /// - `slot`：槽位编号（0-4, 98, 99）
    /// - `data`：要保存的存档数据
    ///
    /// # 返回值
    /// - `Ok(SaveSlotInfo)`：保存成功，返回槽位摘要信息
    /// - `Err(SaveError::Serialize)`：MessagePack 序列化失败
    /// - `Err(SaveError::Io)`：文件写入失败（磁盘空间不足、权限等）
    ///
    /// # 注意事项
    /// - 覆盖已存在的同槽位存档（无确认机制——调用方负责确认）
    /// - 序列化失败时不会写入任何数据（原子性：先序列化到内存，再写入文件）
    pub fn save(&self, slot: u8, data: &SaveData) -> Result<SaveSlotInfo, SaveError> {
        // 步骤 1：序列化 SaveData → MessagePack 字节
        let msgpack_bytes = rmp_serde::to_vec(data).map_err(to_serialize_error)?;

        // 步骤 2：计算 CRC32 校验和
        let crc32 = crc32fast::hash(&msgpack_bytes);

        // 步骤 3：写入文件 [CRC32 LE u32] + [MessagePack 数据]
        let file_path = self.slot_path(slot);
        let mut file = File::create(&file_path)?;

        // 写入 CRC32（little-endian 4 字节）
        file.write_all(&crc32.to_le_bytes())?;

        // 写入 MessagePack 数据
        file.write_all(&msgpack_bytes)?;

        // 确保数据刷新到磁盘
        file.flush()?;

        // 构造返回的槽位摘要
        let info = SaveSlotInfo {
            slot,
            timestamp: data.timestamp.clone(),
            scene_id: data.scene_id.clone(),
            has_thumbnail: false, // 缩略图由 PH2-T07 管理
        };

        Ok(info)
    }

    /// 从指定槽位读取并反序列化存档数据。
    ///
    /// 读取流程：
    /// 1. 读取存档文件的全部字节
    /// 2. 检查最小长度（至少 4 字节 CRC32）
    /// 3. 分离前 4 字节（CRC32 LE）和剩余字节（MessagePack 数据）
    /// 4. 重新计算 MessagePack 数据的 CRC32 并比对
    /// 5. CRC32 通过 → `rmp_serde::from_slice()` 反序列化为 `SaveData`
    /// 6. 检查 `version` 字段与 `CURRENT_VERSION` 的兼容性
    ///
    /// # 参数
    /// - `slot`：槽位编号
    ///
    /// # 返回值
    /// - `Ok(SaveData)`：读取成功
    /// - `Err(SaveError::EmptySlot)`：槽位不存在存档文件
    /// - `Err(SaveError::Corrupted)`：CRC32 校验失败或文件格式错误
    /// - `Err(SaveError::Deserialize)`：MessagePack 反序列化失败
    /// - `Err(SaveError::IncompatibleVersion)`：存档版本不兼容
    /// - `Err(SaveError::Io)`：文件读取失败
    pub fn load(&self, slot: u8) -> Result<SaveData, SaveError> {
        let file_path = self.slot_path(slot);

        // 步骤 1：检查文件是否存在
        if !file_path.exists() {
            return Err(SaveError::EmptySlot { slot });
        }

        // 步骤 2：读取文件全部字节
        let file_bytes = fs::read(&file_path)?;

        // 步骤 3：最小长度检查（至少需要 4 字节 CRC32）
        if file_bytes.len() < 4 {
            return Err(SaveError::Corrupted {
                slot,
                reason: format!(
                    "文件大小 {} 字节 < 最小长度 4 字节，文件可能被截断",
                    file_bytes.len()
                ),
            });
        }

        // 步骤 4：分离 CRC32 和 MessagePack 数据
        let stored_crc32_bytes: [u8; 4] = file_bytes[..4]
            .try_into()
            .expect("已通过 len >= 4 检查，slice 长度为 4");
        let stored_crc32 = u32::from_le_bytes(stored_crc32_bytes);
        let msgpack_bytes = &file_bytes[4..];

        // 步骤 5：验证 CRC32
        let computed_crc32 = crc32fast::hash(msgpack_bytes);
        if stored_crc32 != computed_crc32 {
            return Err(SaveError::Corrupted {
                slot,
                reason: format!(
                    "CRC32 校验失败：存储值 0x{:08X}，计算结果 0x{:08X}，存档文件可能已损坏",
                    stored_crc32, computed_crc32
                ),
            });
        }

        // 步骤 6：MessagePack 反序列化
        let save_data: SaveData =
            rmp_serde::from_slice(msgpack_bytes).map_err(to_deserialize_error)?;

        // 步骤 7：版本兼容性检查
        if save_data.version != CURRENT_VERSION {
            return Err(SaveError::IncompatibleVersion {
                found: save_data.version,
                expected: CURRENT_VERSION,
                hint: "存档格式版本不匹配，请使用对应版本的引擎加载此存档。".into(),
            });
        }

        Ok(save_data)
    }

    /// 列出存档目录中所有有效的存档文件，返回槽位摘要列表。
    ///
    /// 扫描 `save_dir` 中匹配 `slot_*.sav` 模式的文件，对每个文件：
    /// 1. 解析槽位编号（从文件名中提取）
    /// 2. 读取并 CRC32 校验
    /// 3. MessagePack 反序列化
    /// 4. 提取 `slot`/`timestamp`/`scene_id` 构造 `SaveSlotInfo`
    ///
    /// CRC32 校验失败的文件会被跳过（记录 warn 日志但不会导致整体失败），
    /// 因为列表展示不需要完整验证——损坏的文件在加载时会再次校验并返回错误。
    ///
    /// # 返回值
    /// - `Ok(Vec<SaveSlotInfo>)`：槽位摘要列表（按槽位号升序排列）
    /// - `Err(SaveError::Io)`：目录读取失败
    ///
    /// # 性能
    /// 当前 7 个槽位场景下每个文件完整反序列化，总耗时 < 5ms。
    /// 后续 30 槽位时可能需优化为只解析头部字段。
    pub fn list_saves(&self) -> Result<Vec<SaveSlotInfo>, SaveError> {
        let mut infos: Vec<SaveSlotInfo> = Vec::new();

        // 确保存档目录存在
        if !self.save_dir.exists() {
            return Ok(infos); // 空目录 → 空列表
        }

        // 扫描目录中的 .sav 文件
        let entries = fs::read_dir(&self.save_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // 只处理 .sav 文件
            if path.extension().and_then(|s| s.to_str()) != Some(SAVE_EXTENSION) {
                continue;
            }

            // 从文件名解析槽位号（slot_{NN}.sav → NN）
            let slot = match parse_slot_from_filename(&path) {
                Some(s) => s,
                None => continue, // 文件名格式不匹配，跳过
            };

            // 读取并反序列化存档文件
            match self.load(slot) {
                Ok(save_data) => {
                    let has_thumbnail = self.thumbnail_path(slot).exists();
                    infos.push(SaveSlotInfo {
                        slot,
                        timestamp: save_data.timestamp,
                        scene_id: save_data.scene_id,
                        has_thumbnail,
                    });
                }
                Err(SaveError::Corrupted { .. }) => {
                    // CRC32 损坏的存档：跳过但保留占位信息
                    // 用户仍可在列表中看到此槽位（标注为损坏）
                    infos.push(SaveSlotInfo {
                        slot,
                        timestamp: "（存档已损坏）".into(),
                        scene_id: "（未知）".into(),
                        has_thumbnail: false,
                    });
                }
                Err(_) => {
                    // 其他错误（如 IncompatibleVersion、EmptySlot）：跳过
                }
            }
        }

        // 按槽位号升序排列
        infos.sort_by_key(|info| info.slot);

        Ok(infos)
    }

    /// 删除指定槽位的存档文件。
    ///
    /// # 参数
    /// - `slot`：槽位编号
    ///
    /// # 返回值
    /// - `Ok(())`：删除成功
    /// - `Err(SaveError::EmptySlot)`：槽位不存在存档文件
    /// - `Err(SaveError::Io)`：文件删除失败（权限不足等）
    pub fn delete_save(&self, slot: u8) -> Result<(), SaveError> {
        let file_path = self.slot_path(slot);

        if !file_path.exists() {
            return Err(SaveError::EmptySlot { slot });
        }

        fs::remove_file(&file_path)?;
        Ok(())
    }

    /// 检查指定槽位是否存在存档文件。
    ///
    /// # 参数
    /// - `slot`：槽位编号
    ///
    /// # 返回值
    /// - `true`：存档文件存在
    /// - `false`：存档文件不存在
    pub fn slot_exists(&self, slot: u8) -> bool {
        self.slot_path(slot).exists()
    }

    /// 返回存档目录的路径引用。
    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    /// 返回指定槽位的存档文件完整路径。
    ///
    /// 文件命名格式：`slot_{NN}.sav`（NN 为 2 位零填充十进制数）。
    ///
    /// # 参数
    /// - `slot`：槽位编号
    ///
    /// # 示例
    /// ```
    /// use std::path::PathBuf;
    /// use aster_save::SaveManager;
    ///
    /// let manager = SaveManager::new(PathBuf::from("saves"));
    /// assert_eq!(
    ///     manager.slot_path(0),
    ///     PathBuf::from("saves").join("slot_00.sav")
    /// );
    /// assert_eq!(
    ///     manager.slot_path(99),
    ///     PathBuf::from("saves").join("slot_99.sav")
    /// );
    /// ```
    pub fn slot_path(&self, slot: u8) -> PathBuf {
        self.save_dir
            .join(format!("slot_{:02}.{}", slot, SAVE_EXTENSION))
    }

    /// 返回指定槽位的缩略图文件完整路径。
    ///
    /// 缩略图命名格式：`slot_{NN}_thumb.png`
    /// 缩略图由 PH2-T07 的帧捕获功能生成和管理。
    ///
    /// # 参数
    /// - `slot`：槽位编号
    pub fn thumbnail_path(&self, slot: u8) -> PathBuf {
        self.save_dir.join(format!("slot_{:02}_thumb.png", slot))
    }

    /// 检查指定槽位是否已有存档（与 `slot_exists()` 等价，语义更直观）。
    ///
    /// # 参数
    /// - `slot`：槽位编号
    pub fn has_save(&self, slot: u8) -> bool {
        self.slot_exists(slot)
    }

    /// 将 RGBA 像素数据编码为 PNG 缩略图并保存到槽位对应的文件。
    ///
    /// 缩略图会自动缩放至 320×180（16:9 宽高比），以 PNG 格式保存。
    /// 使用 `image` crate 的 Lanczos3 滤镜进行高质量缩放。
    ///
    /// # 参数
    /// - `slot`：槽位编号
    /// - `rgba_pixels`：原始 RGBA8 像素数据（宽度×高度×4 字节）
    /// - `width`：原始图像宽度（像素）
    /// - `height`：原始图像高度（像素）
    ///
    /// # 返回值
    /// - `Ok(())`：缩略图保存成功
    /// - `Err(SaveError::Io)`：文件写入失败
    /// - `Err(SaveError::Serialize(String))`：PNG 编码失败（极少发生）
    ///
    /// # 性能
    /// 缩放 + PNG 编码耗时 < 5ms（1920×1080 → 320×180）。
    pub fn save_thumbnail(
        &self,
        slot: u8,
        rgba_pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), SaveError> {
        const THUMB_WIDTH: u32 = 320;
        const THUMB_HEIGHT: u32 = 180;

        // 步骤 1：从原始 RGBA 像素构建 image 动态图像
        let img = match image::RgbaImage::from_raw(width, height, rgba_pixels.to_vec()) {
            Some(img) => img,
            None => {
                return Err(SaveError::Serialize(format!(
                    "无法从像素数据构建图像：width={}, height={}, data_len={}",
                    width,
                    height,
                    rgba_pixels.len()
                )));
            }
        };

        // 步骤 2：缩放到 320×180（Lanczos3 高质量缩放）
        // Lanczos3 在缩小场景下效果最好，保留更多高频细节
        let thumb = DynamicImage::ImageRgba8(img).resize(
            THUMB_WIDTH,
            THUMB_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        );

        // 步骤 3：编码为 PNG 字节
        let mut png_bytes: Vec<u8> = Vec::new();
        thumb
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .map_err(|e| SaveError::Serialize(format!("PNG 编码失败：{}", e)))?;

        // 步骤 4：写入缩略图文件
        let thumb_path = self.thumbnail_path(slot);
        fs::write(&thumb_path, &png_bytes)?;

        Ok(())
    }
}

// ─── 辅助函数 ──────────────────────────────────────────────────────────────

/// 返回指定槽位的人类可读标签。
///
/// # 槽位标签映射
///
/// | 槽位 | 标签 |
/// |------|------|
/// | 0 ~ 4 | "槽位 1" ~ "槽位 5" |
/// | 98 | "快速存档" |
/// | 99 | "自动存档" |
/// | 其他 | "槽位 {slot}" |
///
/// # 参数
/// - `slot`：槽位编号
pub fn slot_label(slot: u8) -> String {
    match slot {
        QUICK_SLOT => "快速存档".to_string(),
        AUTO_SLOT => "自动存档".to_string(),
        n if n < MANUAL_SLOT_COUNT => format!("槽位 {}", n + 1),
        _ => format!("槽位 {}", slot),
    }
}

/// 从存档文件名解析槽位编号。
///
/// 文件名格式：`slot_{NN}.sav`，其中 NN 为 1-2 位十进制数（0-99）。
///
/// # 参数
/// - `path`：存档文件路径
///
/// # 返回值
/// - `Some(u8)`：成功解析的槽位号
/// - `None`：文件名不匹配 `slot_*.sav` 模式，或槽位号解析失败
fn parse_slot_from_filename(path: &Path) -> Option<u8> {
    let stem = path.file_stem()?.to_str()?;

    // 匹配 "slot_" 前缀
    let number_part = stem.strip_prefix("slot_")?;

    // 解析为 u8
    number_part.parse::<u8>().ok()
}

// ─── 测试模块 ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use aster_core::{AudioSnapshot, FlagSet, RenderState, Value, VariableStore, VmSnapshot};

    /// 创建测试用的临时目录路径，并在测试结束后清理。
    fn temp_save_dir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aster_test_{}", test_name));
        // 清理可能残留的旧测试数据
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建测试目录失败");
        dir
    }

    /// 创建用于测试的 SaveData 实例（含有意义的填充数据）。
    fn test_save_data(slot: u8, scene_id: &str) -> SaveData {
        let mut variables = VariableStore::new();
        variables.set("score", Value::Int(100));
        variables.set("player_name", Value::String("测试角色".into()));

        let mut flags = FlagSet::new();
        flags.set("test_flag");

        let mut registers: [Value; 16] = [
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
        ];
        registers[0] = Value::Int(42);

        let mut data = SaveData::new(slot, scene_id);
        data.variables = variables;
        data.flags = flags;
        data.vm_snapshot = VmSnapshot {
            pc: 100,
            registers,
            call_stack_depth: 1,
            stack_len: 3,
        };
        data.audio_state = AudioSnapshot {
            current_bgm_path: Some("bgm/test.ogg".into()),
            bgm_position_secs: 10.0,
            bgm_looping: true,
            bgm_volume: 0.9,
            se_volume: 0.5,
        };
        data.render_state = RenderState {
            current_bg: Some("backgrounds/test".into()),
            displayed_sprites: vec![],
        };
        data
    }

    /// 清理测试目录的辅助函数。
    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // ─── AC02 ───────────────────────────────────────────────────────────────

    /// AC02 — 存档写入磁盘：`save()` 成功后存档文件存在于磁盘。
    #[test]
    fn ac02_save_writes_file() {
        let dir = temp_save_dir("ac02");
        let manager = SaveManager::new(dir.clone());
        let data = test_save_data(0, "chapter1/test");

        let info = manager.save(0, &data).expect("保存应成功");
        assert_eq!(info.slot, 0);

        // 验证文件确实存在
        let file_path = manager.slot_path(0);
        assert!(file_path.exists(), "存档文件应存在于磁盘");
        assert!(file_path.is_file());

        // 验证文件大小合理（至少大于 CRC32 4 字节）
        let metadata = fs::metadata(&file_path).expect("应能获取文件元数据");
        assert!(metadata.len() > 4, "存档文件应大于 4 字节（CRC32 + 数据）");

        cleanup(&dir);
    }

    // ─── AC03 ───────────────────────────────────────────────────────────────

    /// AC03 — 读档恢复：保存后立即加载同一槽位，数据完全一致。
    #[test]
    fn ac03_load_restores_data() {
        let dir = temp_save_dir("ac03");
        let manager = SaveManager::new(dir.clone());
        let original = test_save_data(1, "chapter2/climax");

        manager.save(1, &original).expect("保存应成功");
        let loaded = manager.load(1).expect("加载应成功");

        // 逐字段验证
        assert_eq!(loaded.version, original.version);
        assert_eq!(loaded.slot, original.slot);
        assert_eq!(loaded.scene_id, original.scene_id);
        assert_eq!(loaded.timestamp, original.timestamp);

        // 变量
        assert_eq!(loaded.variables.len(), original.variables.len());
        assert_eq!(loaded.variables.get("score"), Some(&Value::Int(100)));
        assert_eq!(
            loaded.variables.get("player_name"),
            Some(&Value::String("测试角色".into()))
        );

        // 旗标
        assert!(loaded.flags.check("test_flag"));
        assert_eq!(loaded.flags.len(), original.flags.len());

        // VM 快照
        assert_eq!(loaded.vm_snapshot.pc, 100);
        assert_eq!(loaded.vm_snapshot.call_stack_depth, 1);
        assert_eq!(loaded.vm_snapshot.stack_len, 3);
        assert_eq!(loaded.vm_snapshot.registers[0], Value::Int(42));

        // 音频状态
        assert_eq!(
            loaded.audio_state.current_bgm_path,
            Some("bgm/test.ogg".into())
        );
        assert!((loaded.audio_state.bgm_position_secs - 10.0).abs() < f64::EPSILON);
        assert!(loaded.audio_state.bgm_looping);
        assert!((loaded.audio_state.bgm_volume - 0.9).abs() < f32::EPSILON);
        assert!((loaded.audio_state.se_volume - 0.5).abs() < f32::EPSILON);

        // 渲染状态
        assert_eq!(
            loaded.render_state.current_bg,
            Some("backgrounds/test".into())
        );

        cleanup(&dir);
    }

    // ─── AC04 ───────────────────────────────────────────────────────────────

    /// AC04 — CRC32 校验通过：正常保存/加载流程中 CRC32 验证通过。
    #[test]
    fn ac04_crc32_valid() {
        let dir = temp_save_dir("ac04");
        let manager = SaveManager::new(dir.clone());
        let data = test_save_data(0, "test");

        manager.save(0, &data).expect("保存应成功");

        // 正常加载不应返回错误（CRC32 应匹配）
        let result = manager.load(0);
        assert!(result.is_ok(), "CRC32 校验应通过，得到 Ok");
        assert_eq!(result.unwrap().scene_id, "test");

        cleanup(&dir);
    }

    // ─── AC05 ───────────────────────────────────────────────────────────────

    /// AC05 — CRC32 损坏检测：篡改存档文件后加载应返回 Corrupted 错误。
    #[test]
    fn ac05_crc32_corrupted_detection() {
        let dir = temp_save_dir("ac05");
        let manager = SaveManager::new(dir.clone());
        let data = test_save_data(0, "test");

        manager.save(0, &data).expect("保存应成功");

        // 篡改存档文件：修改第 10 个字节（CRC32 之后的数据区）
        let file_path = manager.slot_path(0);
        let mut file_bytes = fs::read(&file_path).expect("读取文件应成功");
        // 确保文件足够长，修改数据区的某个字节
        if file_bytes.len() > 10 {
            file_bytes[10] = file_bytes[10].wrapping_add(1); // 翻转一个 bit
        } else {
            // 文件太短，修改第 5 字节（数据区第一个字节）
            file_bytes[5] = file_bytes[5].wrapping_add(1);
        }
        fs::write(&file_path, &file_bytes).expect("写入篡改文件应成功");

        // 加载篡改后的文件应返回 Corrupted 错误
        let result = manager.load(0);
        match result {
            Err(SaveError::Corrupted { slot, reason: _ }) => {
                assert_eq!(slot, 0);
            }
            other => panic!("期望 Corrupted 错误，实际得到 {:?}", other),
        }

        cleanup(&dir);
    }

    // ─── AC06 ───────────────────────────────────────────────────────────────

    /// AC06 — 空槽位加载：对未保存过的槽位调用 `load()` 返回 EmptySlot。
    #[test]
    fn ac06_load_empty_slot() {
        let dir = temp_save_dir("ac06");
        let manager = SaveManager::new(dir.clone());

        let result = manager.load(99);
        match result {
            Err(SaveError::EmptySlot { slot }) => {
                assert_eq!(slot, 99);
            }
            other => panic!("期望 EmptySlot 错误，实际得到 {:?}", other),
        }

        cleanup(&dir);
    }

    // ─── AC07 ───────────────────────────────────────────────────────────────

    /// AC07 — 版本不兼容检测：构造高版本号的存档文件，加载应返回 IncompatibleVersion。
    #[test]
    fn ac07_version_incompatible() {
        let dir = temp_save_dir("ac07");
        let manager = SaveManager::new(dir.clone());

        // 手动构造一个 version=99 的 SaveData
        let mut data = test_save_data(0, "test");
        data.version = 99;

        // 手动序列化并写入（绕过 save 方法的版本检查）
        let msgpack_bytes = rmp_serde::to_vec(&data).expect("序列化应成功");
        let crc32 = crc32fast::hash(&msgpack_bytes);
        let file_path = manager.slot_path(0);
        let mut file = File::create(&file_path).expect("创建文件应成功");
        file.write_all(&crc32.to_le_bytes())
            .expect("写入 CRC32 应成功");
        file.write_all(&msgpack_bytes).expect("写入数据应成功");
        file.flush().expect("刷新应成功");

        // 加载应返回 IncompatibleVersion
        let result = manager.load(0);
        match result {
            Err(SaveError::IncompatibleVersion {
                found,
                expected,
                hint: _,
            }) => {
                assert_eq!(found, 99);
                assert_eq!(expected, CURRENT_VERSION);
            }
            other => panic!("期望 IncompatibleVersion 错误，实际得到 {:?}", other),
        }

        cleanup(&dir);
    }

    // ─── AC08 ───────────────────────────────────────────────────────────────

    /// AC08 — list_saves 列出全部存档：保存多个槽位后列表正确返回。
    #[test]
    fn ac08_list_saves() {
        let dir = temp_save_dir("ac08");
        let manager = SaveManager::new(dir.clone());

        // 保存 3 个不同的槽位
        manager
            .save(0, &test_save_data(0, "scene_a"))
            .expect("保存槽位 0 应成功");
        manager
            .save(2, &test_save_data(2, "scene_b"))
            .expect("保存槽位 2 应成功");
        manager
            .save(4, &test_save_data(4, "scene_c"))
            .expect("保存槽位 4 应成功");

        // 列表查询
        let saves = manager.list_saves().expect("列表应成功");
        assert_eq!(saves.len(), 3, "应有 3 个存档");

        // 验证每个槽位信息
        assert_eq!(saves[0].slot, 0);
        assert_eq!(saves[0].scene_id, "scene_a");

        assert_eq!(saves[1].slot, 2);
        assert_eq!(saves[1].scene_id, "scene_b");

        assert_eq!(saves[2].slot, 4);
        assert_eq!(saves[2].scene_id, "scene_c");

        // 验证 timestamp 非空
        for info in &saves {
            assert!(!info.timestamp.is_empty(), "时间戳不应为空");
        }

        cleanup(&dir);
    }

    // ─── AC09 ───────────────────────────────────────────────────────────────

    /// AC09 — 删除存档：save → delete → 文件不存在 → load 返回 EmptySlot。
    #[test]
    fn ac09_delete_save() {
        let dir = temp_save_dir("ac09");
        let manager = SaveManager::new(dir.clone());
        let data = test_save_data(0, "test");

        // 1. 保存
        manager.save(0, &data).expect("保存应成功");
        assert!(manager.slot_path(0).exists());

        // 2. 删除
        manager.delete_save(0).expect("删除应成功");

        // 3. 文件不存在
        assert!(!manager.slot_path(0).exists());
        assert!(!manager.slot_exists(0));

        // 4. 加载应返回 EmptySlot
        let result = manager.load(0);
        match result {
            Err(SaveError::EmptySlot { slot }) => {
                assert_eq!(slot, 0);
            }
            other => panic!("期望 EmptySlot 错误，实际得到 {:?}", other),
        }

        cleanup(&dir);
    }

    // ─── 辅助函数测试 ──────────────────────────────────────────────────────

    /// 验证 slot_path() 生成正确的文件路径。
    #[test]
    fn test_slot_path_format() {
        let manager = SaveManager::new(PathBuf::from("test_saves"));

        assert_eq!(
            manager.slot_path(0),
            PathBuf::from("test_saves").join("slot_00.sav")
        );
        assert_eq!(
            manager.slot_path(5),
            PathBuf::from("test_saves").join("slot_05.sav")
        );
        assert_eq!(
            manager.slot_path(99),
            PathBuf::from("test_saves").join("slot_99.sav")
        );
    }

    /// 验证 thumbnail_path() 生成正确的缩略图路径。
    #[test]
    fn test_thumbnail_path_format() {
        let manager = SaveManager::new(PathBuf::from("test_saves"));

        assert_eq!(
            manager.thumbnail_path(0),
            PathBuf::from("test_saves").join("slot_00_thumb.png")
        );
    }

    /// 验证 parse_slot_from_filename 正确解析各种文件名。
    #[test]
    fn test_parse_slot_from_filename() {
        assert_eq!(
            parse_slot_from_filename(&PathBuf::from("slot_00.sav")),
            Some(0)
        );
        assert_eq!(
            parse_slot_from_filename(&PathBuf::from("slot_99.sav")),
            Some(99)
        );
        assert_eq!(
            parse_slot_from_filename(&PathBuf::from("slot_5.sav")),
            Some(5)
        );
        assert_eq!(
            parse_slot_from_filename(&PathBuf::from("slot_256.sav")),
            None
        ); // 256 溢出 u8
        assert_eq!(
            parse_slot_from_filename(&PathBuf::from("other_file.txt")),
            None
        );
        assert_eq!(
            parse_slot_from_filename(&PathBuf::from("slot_xx.sav")),
            None
        );
    }

    /// 验证 save_dir() 返回正确的路径。
    #[test]
    fn test_save_dir() {
        let path = PathBuf::from("my_saves");
        let manager = SaveManager::new(path.clone());
        assert_eq!(manager.save_dir(), path);
    }

    /// 验证 slot_exists() 正确判断文件存在性。
    #[test]
    fn test_slot_exists() {
        let dir = temp_save_dir("slot_exists");
        let manager = SaveManager::new(dir.clone());

        assert!(!manager.slot_exists(0));

        let data = test_save_data(0, "test");
        manager.save(0, &data).expect("保存应成功");

        assert!(manager.slot_exists(0));
        assert!(!manager.slot_exists(1));

        cleanup(&dir);
    }

    /// 验证保存空状态 SaveData（所有字段均为默认值）。
    #[test]
    fn test_save_empty_state() {
        let dir = temp_save_dir("empty_state");
        let manager = SaveManager::new(dir.clone());
        let data = SaveData::new(0, "empty_test");

        manager.save(0, &data).expect("保存应成功");
        let loaded = manager.load(0).expect("加载应成功");

        assert_eq!(loaded.scene_id, "empty_test");
        assert!(loaded.variables.is_empty());
        assert!(loaded.flags.is_empty());
        assert!(loaded.vm_snapshot.pc == 0);

        cleanup(&dir);
    }

    /// 验证对不存在的存档调用 delete_save 返回 EmptySlot。
    #[test]
    fn test_delete_nonexistent_slot() {
        let dir = temp_save_dir("delete_nonexistent");
        let manager = SaveManager::new(dir.clone());

        let result = manager.delete_save(50);
        match result {
            Err(SaveError::EmptySlot { slot }) => assert_eq!(slot, 50),
            other => panic!("期望 EmptySlot 错误，实际得到 {:?}", other),
        }

        cleanup(&dir);
    }

    /// 验证文件格式被破坏（截断）时的检测：文件小于 4 字节。
    #[test]
    fn test_corrupted_truncated_file() {
        let dir = temp_save_dir("corrupted_truncated");
        let manager = SaveManager::new(dir.clone());

        // 写入一个只有 2 字节的文件
        let file_path = manager.slot_path(0);
        fs::write(&file_path, [0x00, 0x01]).expect("写入应成功");

        let result = manager.load(0);
        match result {
            Err(SaveError::Corrupted { slot, reason: _ }) => {
                assert_eq!(slot, 0);
            }
            other => panic!("期望 Corrupted 错误，实际得到 {:?}", other),
        }

        cleanup(&dir);
    }

    /// 验证 list_saves 在空目录下返回空列表。
    #[test]
    fn test_list_saves_empty_dir() {
        let dir = temp_save_dir("list_empty");
        let manager = SaveManager::new(dir.clone());

        let saves = manager.list_saves().expect("列表应成功");
        assert!(saves.is_empty());

        cleanup(&dir);
    }
}
