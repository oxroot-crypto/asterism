//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-asset/src/asset_manager.rs
//! 功能概述：资源管理器 — `AssetManager` 是资源管理系统的中枢，负责：
//!           1. 扫描项目 `assets/` 目录，建立资源索引（AssetId ↔ 文件路径）
//!           2. 管理可扩展的 `AssetLoader` 注册表（按资源类型分发）
//!           3. 提供统一的资源加载入口（查元数据→找加载器→解码）
//!           4. 支持按 ID 或路径查询资源元数据
//!           本模块不实现缓存（PH2-T05）——每次 `load()` 调用都重新解码。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::{AssetId, AssetType, Asset}（核心资源类型）
//! - crate::error::AssetError（错误类型）
//! - crate::loader::{AssetLoader, LoadedAsset}（加载器 trait + 统一数据表示）
//!
//! 对应任务：PH2-T04 — aster-asset 资源加载基础设施
//! 后续扩展：PH2-T05 — LRU 缓存（修改 load() 方法添加缓存检查）

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aster_core::{AssetId, AssetType};

use crate::error::AssetError;
use crate::loader::{AssetLoader, LoadedAsset};

// ============================================================================
// AssetMetadata — 资源元数据（轻量索引条目）
// ============================================================================

/// 资源元数据 — 描述单个资源的标识、类型和磁盘位置。
///
/// 与 `aster_core::Asset` 的区别：
/// - `AssetMetadata` 仅包含扫描器可自动获取的信息（无 `metadata` HashMap）
/// - 字段更精简，适合作为 `AssetManager` 内部索引条目
/// - `file_size` 用于缓存层的内存预估（PH2-T05）
///
/// # 示例
/// ```
/// use aster_asset::AssetMetadata;
/// use aster_core::{AssetId, AssetType};
/// use std::path::PathBuf;
///
/// let meta = AssetMetadata {
///     id: AssetId(1),
///     asset_type: AssetType::Background,
///     relative_path: PathBuf::from("assets/bg/classroom.png"),
///     file_size: 1048576,
/// };
/// assert_eq!(meta.id, AssetId(1));
/// assert_eq!(meta.asset_type, AssetType::Background);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AssetMetadata {
    /// 资源唯一标识符（自增分配）
    pub id: AssetId,
    /// 资源类型（背景/立绘/音乐/音效/语音/字体/视频/GUI 元素）
    pub asset_type: AssetType,
    /// 相对于 `base_path`（项目根目录）的文件路径
    /// 例如 `assets/bg/classroom.png`、`assets/bgm/theme.ogg`
    pub relative_path: PathBuf,
    /// 文件大小（字节），用于缓存层内存预估
    pub file_size: u64,
}

// ============================================================================
// AssetManager — 资源管理中枢
// ============================================================================

/// 资源管理器 — 游戏资源的统一入口。
///
/// 职责：
/// - **索引**：`scan_assets()` 遍历 `assets/` 目录建立完整索引
/// - **分发**：`load()` 根据资源类型分发到对应的 `AssetLoader` 实现
/// - **查询**：支持按 `AssetId` 或文件路径查找资源元数据
///
/// # 生命周期
///
/// ```text
/// AssetManager::new(base_path)
///   → scan_assets()           // 扫描目录，建立索引
///   → register_loader(loader) // 注册加载器（至少需要 TextureLoader + AudioLoader）
///   → load(id)               // 加载资源（后续 PH2-T05 添加缓存）
/// ```
///
/// # 设计约束
///
/// - 不在本 crate 中创建 wgpu 设备——TextureLoader 通过构造函数注入
/// - 不实现 LRU 缓存（PH2-T05）
/// - 不支持运行时自动重扫描（需手动调用 `scan_assets()`）
///
/// # 使用示例
///
/// ```rust,ignore
/// use aster_asset::{AssetManager, TextureLoader, AudioLoader};
/// use std::sync::Arc;
///
/// let mut manager = AssetManager::new("/path/to/project");
/// manager.scan_assets()?;
/// manager.register_loader(Arc::new(TextureLoader::new(device, queue)));
/// manager.register_loader(Arc::new(AudioLoader::new()));
///
/// if let Some(id) = manager.find_by_path(Path::new("assets/bg/classroom.png")) {
///     let asset = manager.load(id)?;
/// }
/// ```
pub struct AssetManager {
    /// 项目根目录（`assets/` 的父目录）
    base_path: PathBuf,
    /// 资源索引表：AssetId → AssetMetadata
    assets: HashMap<AssetId, AssetMetadata>,
    /// 路径反向索引：相对路径 → AssetId
    path_to_id: HashMap<PathBuf, AssetId>,
    /// 资源类型 → 加载器映射表
    loaders: HashMap<AssetType, Arc<dyn AssetLoader>>,
    /// 自增 ID 计数器（每次分配 +1）
    next_id: u64,
}

impl AssetManager {
    /// 创建新的资源管理器。
    ///
    /// # 参数
    /// - `base_path`：项目根目录的绝对路径。`assets/` 子目录应位于 `base_path/assets/`。
    ///
    /// # 返回值
    /// 返回空的 `AssetManager`——需调用 `scan_assets()` 建立索引，
    /// 并 `register_loader()` 注册至少一个加载器后才能加载资源。
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            assets: HashMap::new(),
            path_to_id: HashMap::new(),
            loaders: HashMap::new(),
            next_id: 1, // 从 1 开始，0 预留为无效 ID
        }
    }

    // ─── 资源扫描 ───────────────────────────────────────────────────────

    /// 扫描 `assets/` 目录，建立完整资源索引。
    ///
    /// 遍历 `assets/{bg,char,bgm,se,voice,font,video,gui}` 各子目录，
    /// 按文件扩展名过滤有效资源文件，为每个文件分配唯一 `AssetId`。
    ///
    /// # 扫描规则
    ///
    /// - **目录映射**：子目录名决定 `AssetType`（如 `bg/` → `Background`）
    /// - **扩展名过滤**：仅收集各类型支持的扩展名（如 `bg/` 仅 .png/.webp/.jpg）
    /// - **跳过**：`.aster_cache/` 目录、隐藏文件（`.` 开头）、符号链接
    /// - **去重**：同一文件多次扫描不会重复分配 ID（幂等操作）
    ///
    /// # 返回值
    /// - `Ok(count)`：成功扫描 `count` 个资源文件
    /// - `Err(AssetError::Io)`：目录不存在或无法读取
    ///
    /// # 警告
    /// 此方法会**追加**到现有索引。如需重建索引，
    /// 创建新的 `AssetManager` 实例。
    pub fn scan_assets(&mut self) -> Result<usize, AssetError> {
        let assets_dir = self.base_path.join("assets");

        if !assets_dir.exists() {
            return Err(AssetError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("assets 目录不存在：{}", assets_dir.display()),
            )));
        }

        if !assets_dir.is_dir() {
            return Err(AssetError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("assets 路径不是目录：{}", assets_dir.display()),
            )));
        }

        let mut count = 0;

        // 遍历每种资源类型对应的子目录
        for (dir_name, asset_type) in Self::asset_dir_mappings() {
            let sub_dir = assets_dir.join(dir_name);
            if !sub_dir.is_dir() {
                continue; // 子目录不存在则跳过（允许部分目录缺失）
            }

            // 获取该类型的有效扩展名列表
            let valid_extensions = Self::valid_extensions_for(asset_type);

            // 遍历子目录中的所有文件
            let entries = match std::fs::read_dir(&sub_dir) {
                Ok(entries) => entries,
                Err(e) => {
                    // 读取目录失败时记录但继续（不阻断整个扫描）
                    eprintln!(
                        "[aster-asset] 警告：无法读取目录 {}：{e}",
                        sub_dir.display()
                    );
                    continue;
                }
            };

            for entry in entries.flatten() {
                let file_path = entry.path();

                // 跳过目录和隐藏文件
                if file_path.is_dir() {
                    continue;
                }

                if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str())
                    && file_name.starts_with('.')
                {
                    continue; // 跳过隐藏文件
                }

                // 检查扩展名是否合法
                let extension = file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();

                if !valid_extensions.contains(&extension.as_str()) {
                    continue; // 不支持的扩展名
                }

                // 获取文件大小
                let file_size = match std::fs::metadata(&file_path) {
                    Ok(meta) => meta.len(),
                    Err(_) => continue, // 无法读取元数据则跳过
                };

                // 计算相对路径（相对于 base_path）
                let relative_path = match file_path.strip_prefix(&self.base_path) {
                    Ok(p) => p.to_path_buf(),
                    Err(_) => continue,
                };

                // 如果该路径已经索引，跳过（幂等扫描）
                if self.path_to_id.contains_key(&relative_path) {
                    continue;
                }

                // 分配 ID 并插入索引
                let id = AssetId(self.next_id);
                self.next_id += 1;

                let metadata = AssetMetadata {
                    id,
                    asset_type: (*asset_type).clone(),
                    relative_path: relative_path.clone(),
                    file_size,
                };

                self.assets.insert(id, metadata);
                self.path_to_id.insert(relative_path, id);
                count += 1;
            }
        }

        Ok(count)
    }

    /// 返回资源类型→子目录名的映射表。
    ///
    /// 与 `AssetType::dir_name()` 保持一致，用于扫描时确定遍历目标。
    fn asset_dir_mappings() -> &'static [(&'static str, &'static AssetType)] {
        &[
            ("bg", &AssetType::Background),
            ("char", &AssetType::CharacterSprite),
            ("bgm", &AssetType::Bgm),
            ("se", &AssetType::Se),
            ("voice", &AssetType::Voice),
            ("font", &AssetType::Font),
            ("video", &AssetType::Video),
            ("gui", &AssetType::GuiElement),
        ]
    }

    /// 返回指定资源类型的有效文件扩展名列表。
    ///
    /// 仅当文件位于对应子目录且扩展名在此列表中时，才被识别为该类型。
    fn valid_extensions_for(asset_type: &AssetType) -> &'static [&'static str] {
        match asset_type {
            AssetType::Background => &["png", "webp", "jpg", "jpeg"],
            AssetType::CharacterSprite => &["png", "webp"],
            AssetType::Bgm => &["ogg", "flac", "mp3", "wav"],
            AssetType::Se => &["ogg", "wav"],
            AssetType::Voice => &["ogg", "wav"],
            AssetType::Font => &["ttf", "otf"],
            AssetType::Video => &["webm", "mp4"],
            AssetType::GuiElement => &["png", "webp"],
        }
    }

    // ─── 加载器注册 ─────────────────────────────────────────────────────

    /// 注册资源加载器。
    ///
    /// 调用 `loader.supported_types()` 获取支持的类型列表，
    /// 将加载器注册到所有对应类型。如果某类型已有注册的加载器，
    /// 则覆盖旧加载器（最后一次注册生效）。
    ///
    /// # 参数
    /// - `loader`：通过 `Arc` 共享的加载器实例。`Arc` 允许同一加载器
    ///   注册到多个资源类型（共享引用）。
    ///
    /// # 示例
    /// ```rust,ignore
    /// manager.register_loader(Arc::new(TextureLoader::new(device, queue)));
    /// manager.register_loader(Arc::new(AudioLoader::new()));
    /// ```
    pub fn register_loader(&mut self, loader: Arc<dyn AssetLoader>) {
        for asset_type in loader.supported_types() {
            self.loaders.insert(asset_type.clone(), Arc::clone(&loader));
        }
    }

    // ─── 资源加载 ───────────────────────────────────────────────────────

    /// 加载资源——查元数据→找加载器→解码。
    ///
    /// # 流程
    /// 1. 根据 `AssetId` 查找元数据（不存在则返回 `NotFound`）
    /// 2. 根据元数据的 `asset_type` 查找对应加载器（无加载器则 `UnsupportedFormat`）
    /// 3. 拼接完整文件路径，调用加载器的 `load()` 方法
    /// 4. 返回 `LoadedAsset`（PH2-T05 将在此步骤添加缓存检查）
    ///
    /// # 参数
    /// - `id`：要加载的资源标识符
    ///
    /// # 返回值
    /// - `Ok(LoadedAsset)`：解码成功
    /// - `Err(AssetError::NotFound)`：ID 未在索引中
    /// - `Err(AssetError::UnsupportedFormat)`：无对应加载器
    /// - `Err(AssetError::DecodeError)`：解码失败
    ///
    /// # 注意
    /// 当前版本每次调用都重新解码（无缓存）。PH2-T05 将添加 LRU 缓存层，
    /// 届时此方法将先查缓存再解码。
    pub fn load(&self, id: AssetId) -> Result<LoadedAsset, AssetError> {
        // 步骤 1：查找元数据
        let metadata = self.assets.get(&id).ok_or_else(|| AssetError::NotFound {
            path: format!("AssetId({})", id.0),
        })?;

        // 步骤 2：查找加载器
        let loader = self.loaders.get(&metadata.asset_type).ok_or_else(|| {
            AssetError::UnsupportedFormat {
                path: metadata.relative_path.display().to_string(),
                format: format!("{:?}", metadata.asset_type),
            }
        })?;

        // 步骤 3：拼接完整路径并加载
        let full_path = self.base_path.join(&metadata.relative_path);
        loader.load(&full_path)
    }

    // ─── 资源查询 ───────────────────────────────────────────────────────

    /// 按 ID 查询资源元数据。
    ///
    /// # 返回值
    /// - `Some(&AssetMetadata)`：资源存在于索引中
    /// - `None`：未找到该 ID
    pub fn get_metadata(&self, id: AssetId) -> Option<&AssetMetadata> {
        self.assets.get(&id)
    }

    /// 按相对路径查询 AssetId（反向索引）。
    ///
    /// # 参数
    /// - `path`：相对于项目根目录的文件路径（如 `assets/bg/classroom.png`）
    ///
    /// # 返回值
    /// - `Some(AssetId)`：路径存在于索引中
    /// - `None`：未找到该路径
    pub fn find_by_path(&self, path: &Path) -> Option<AssetId> {
        self.path_to_id.get(path).copied()
    }

    /// 返回所有已索引资源的元数据迭代器。
    ///
    /// 遍历顺序由 `HashMap` 决定（非确定性）。
    /// 如需排序，调用方自行收集并排序。
    pub fn assets(&self) -> impl Iterator<Item = &AssetMetadata> {
        self.assets.values()
    }

    /// 返回当前已索引的资源总数。
    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    /// 返回项目根目录的只读引用。
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// 根据 AssetId 解析完整的文件系统路径。
    ///
    /// # 返回值
    /// - `Some(PathBuf)`：`base_path / relative_path`
    /// - `None`：ID 未在索引中
    pub fn resolve_path(&self, id: AssetId) -> Option<PathBuf> {
        self.assets
            .get(&id)
            .map(|m| self.base_path.join(&m.relative_path))
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01, AC02, AC06, AC07, AC08
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ========================================================================
    // 测试辅助：创建带测试文件的临时项目结构
    // ========================================================================

    /// 创建模拟的项目目录结构，包含 assets/ 子目录和测试文件。
    ///
    /// 创建的目录结构：
    /// ```text
    /// base_path/
    ///   assets/
    ///     bg/
    ///       background.png
    ///       background.webp
    ///     char/
    ///       hero.png
    ///     bgm/
    ///       theme.ogg
    ///     se/
    ///       click.wav
    ///     font/
    ///       default.ttf
    /// ```
    struct TestProject {
        /// 临时目录（Drop 时自动清理）
        _dir: tempfile::TempDir,
        /// 项目根目录路径
        base_path: PathBuf,
    }

    impl TestProject {
        /// 创建测试项目并填充测试文件。
        fn new() -> Self {
            let dir = tempfile::TempDir::new().expect("创建临时目录失败");
            let base_path = dir.path().to_path_buf();

            // 创建 assets 子目录结构
            let sub_dirs = ["bg", "char", "bgm", "se", "font"];
            for sub in &sub_dirs {
                fs::create_dir_all(base_path.join("assets").join(sub)).expect("创建测试子目录失败");
            }

            // 创建测试文件（空文件即可，扫描不检查内容）
            let test_files = [
                "assets/bg/background.png",
                "assets/bg/background.webp",
                "assets/char/hero.png",
                "assets/bgm/theme.ogg",
                "assets/se/click.wav",
                "assets/font/default.ttf",
            ];

            for file in &test_files {
                let path = base_path.join(file);
                fs::write(&path, b"test content").expect("写入测试文件失败");
            }

            Self {
                _dir: dir,
                base_path,
            }
        }
    }

    // ========================================================================
    // AC01 — AssetManager 初始化并扫描目录
    // ========================================================================

    /// AC01: 验证 `scan_assets()` 正确扫描测试项目中的所有资源文件。
    #[test]
    fn ac01_scan_assets_indexes_all_files() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);

        let count = manager.scan_assets().expect("扫描测试项目 assets 应成功");

        assert!(count >= 6, "至少应扫描到 6 个测试文件，实际：{count}");
        assert_eq!(manager.asset_count(), count, "asset_count 应与扫描计数一致");
    }

    /// AC01 补充：验证每个扫描到的资源有唯一 ID。
    #[test]
    fn ac01_scanned_assets_have_unique_ids() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        let mut ids: Vec<AssetId> = manager.assets().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();

        assert_eq!(
            ids.len(),
            manager.asset_count(),
            "所有 AssetId 应唯一（去重后数量不变）"
        );
    }

    /// AC01 补充：验证 assets 目录不存在时返回错误。
    #[test]
    fn ac01_scan_missing_assets_dir_returns_error() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");
        let mut manager = AssetManager::new(dir.path());

        let result = manager.scan_assets();
        assert!(result.is_err(), "assets 目录不存在应返回错误");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("不存在") || err_msg.contains("exist"),
            "错误消息应提示目录不存在：{err_msg}"
        );
    }

    /// AC01 补充：验证空 assets 目录扫描返回 0。
    #[test]
    fn ac01_scan_empty_assets_dir_returns_zero() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");
        fs::create_dir_all(dir.path().join("assets")).expect("创建空 assets 目录");

        let mut manager = AssetManager::new(dir.path());
        let count = manager.scan_assets().expect("扫描空 assets 目录应成功");

        assert_eq!(count, 0, "空目录扫描结果应为 0");
        assert_eq!(manager.asset_count(), 0);
    }

    // ========================================================================
    // AC02 — 资源类型推断正确
    // ========================================================================

    /// AC02: 验证文件在对应子目录中被正确推断类型。
    #[test]
    fn ac02_asset_type_inference() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        // 验证 bg/ → Background
        let bg_id = manager
            .find_by_path(Path::new("assets/bg/background.png"))
            .expect("background.png 应在索引中");
        let bg_meta = manager.get_metadata(bg_id).expect("元数据应存在");
        assert_eq!(
            bg_meta.asset_type,
            AssetType::Background,
            "bg/ 目录中的文件应为 Background 类型"
        );

        // 验证 char/ → CharacterSprite
        let char_id = manager
            .find_by_path(Path::new("assets/char/hero.png"))
            .expect("hero.png 应在索引中");
        let char_meta = manager.get_metadata(char_id).expect("元数据应存在");
        assert_eq!(
            char_meta.asset_type,
            AssetType::CharacterSprite,
            "char/ 目录中的文件应为 CharacterSprite 类型"
        );

        // 验证 bgm/ → Bgm
        let bgm_id = manager
            .find_by_path(Path::new("assets/bgm/theme.ogg"))
            .expect("theme.ogg 应在索引中");
        let bgm_meta = manager.get_metadata(bgm_id).expect("元数据应存在");
        assert_eq!(
            bgm_meta.asset_type,
            AssetType::Bgm,
            "bgm/ 目录中的文件应为 Bgm 类型"
        );

        // 验证 se/ → Se
        let se_id = manager
            .find_by_path(Path::new("assets/se/click.wav"))
            .expect("click.wav 应在索引中");
        let se_meta = manager.get_metadata(se_id).expect("元数据应存在");
        assert_eq!(
            se_meta.asset_type,
            AssetType::Se,
            "se/ 目录中的文件应为 Se 类型"
        );

        // 验证 font/ → Font
        let font_id = manager
            .find_by_path(Path::new("assets/font/default.ttf"))
            .expect("default.ttf 应在索引中");
        let font_meta = manager.get_metadata(font_id).expect("元数据应存在");
        assert_eq!(
            font_meta.asset_type,
            AssetType::Font,
            "font/ 目录中的文件应为 Font 类型"
        );
    }

    /// AC02 补充：验证不支持的扩展名被正确忽略。
    #[test]
    fn ac02_unsupported_extensions_are_skipped() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");
        let assets_bg = dir.path().join("assets").join("bg");
        fs::create_dir_all(&assets_bg).expect("创建 bg 目录");

        // 创建合法文件
        fs::write(assets_bg.join("valid.png"), b"data").expect("写入合法文件");
        // 创建不合法文件
        fs::write(assets_bg.join("invalid.txt"), b"data").expect("写入不合法文件");
        fs::write(assets_bg.join("invalid.exe"), b"data").expect("写入不合法文件");

        let mut manager = AssetManager::new(dir.path());
        let count = manager.scan_assets().expect("扫描应成功");

        assert_eq!(count, 1, "只有 valid.png 应被索引，.txt 和 .exe 应被忽略");
        assert!(
            manager
                .find_by_path(Path::new("assets/bg/valid.png"))
                .is_some(),
            "valid.png 应在索引中"
        );
        assert!(
            manager
                .find_by_path(Path::new("assets/bg/invalid.txt"))
                .is_none(),
            "invalid.txt 不应在索引中"
        );
    }

    // ========================================================================
    // AC06 — 文件不存在返回错误
    // ========================================================================

    /// AC06: 验证加载未索引的 AssetId 返回 NotFound。
    #[test]
    fn ac06_load_nonexistent_id_returns_not_found() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        // 使用一个远超扫描范围的 ID
        let nonexistent_id = AssetId(99999);
        let result = manager.load(nonexistent_id);

        assert!(result.is_err(), "不存在的 AssetId 加载应返回错误");

        match result.unwrap_err() {
            AssetError::NotFound { path } => {
                assert!(path.contains("99999"), "错误信息应包含 ID 信息：{path}");
            }
            other => panic!("预期 NotFound 错误，实际得到：{other:?}"),
        }
    }

    /// AC06 补充：验证未注册加载器的资源类型返回 UnsupportedFormat。
    #[test]
    fn ac06_load_without_loader_returns_unsupported() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        // 未注册任何加载器
        let bg_id = manager
            .find_by_path(Path::new("assets/bg/background.png"))
            .expect("background.png 应在索引中");

        let result = manager.load(bg_id);
        assert!(result.is_err(), "未注册加载器时应返回错误");

        match result.unwrap_err() {
            AssetError::UnsupportedFormat { .. } => {} // 预期
            other => panic!("预期 UnsupportedFormat，实际得到：{other:?}"),
        }
    }

    // ========================================================================
    // AC07 — 不支持的格式返回错误
    // ========================================================================

    /// AC07: 验证注册了仅支持 Bgm 的加载器后，请求 Background 类型资源返回错误。
    #[test]
    fn ac07_unsupported_format_error() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        // 注册一个仅支持 Bgm 的 mock 加载器
        struct BgmOnlyLoader;
        impl AssetLoader for BgmOnlyLoader {
            fn supported_types(&self) -> &[AssetType] {
                &[AssetType::Bgm]
            }
            fn load(&self, _path: &Path) -> Result<LoadedAsset, AssetError> {
                Ok(LoadedAsset::Bytes { data: vec![] })
            }
        }

        manager.register_loader(Arc::new(BgmOnlyLoader));

        // 尝试加载 Background 类型（无对应加载器）
        let bg_id = manager
            .find_by_path(Path::new("assets/bg/background.png"))
            .expect("background.png 应在索引中");

        let result = manager.load(bg_id);
        assert!(result.is_err(), "无 Background 加载器时应返回错误");

        match result.unwrap_err() {
            AssetError::UnsupportedFormat { path, format } => {
                assert!(path.contains("background.png"), "错误路径应包含文件名");
                assert!(
                    format.contains("Background"),
                    "错误格式应提示 Background 类型：{format}"
                );
            }
            other => panic!("预期 UnsupportedFormat，实际得到：{other:?}"),
        }
    }

    // ========================================================================
    // AC08 — 路径→ID 反向查询
    // ========================================================================

    /// AC08: 验证 `find_by_path()` 正确返回 AssetId。
    #[test]
    fn ac08_find_by_path_returns_correct_id() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        // 正向：路径 → ID
        let bg_id = manager.find_by_path(Path::new("assets/bg/background.png"));
        assert!(bg_id.is_some(), "已索引的路径应能查到 ID");

        let bg_id = bg_id.unwrap();
        let meta = manager.get_metadata(bg_id).expect("通过 ID 应能查到元数据");
        assert_eq!(
            meta.relative_path,
            PathBuf::from("assets/bg/background.png")
        );

        // 反向：不存在的路径 → None
        let nonexistent = manager.find_by_path(Path::new("assets/bg/ghost.png"));
        assert!(nonexistent.is_none(), "不存在的路径应返回 None");
    }

    /// AC08 补充：验证使用不同路径格式（含 ./ 或反斜杠）时的查询行为。
    #[test]
    fn ac08_find_by_path_exact_match() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        // 查询使用精确的相对路径
        let id = manager.find_by_path(Path::new("assets/se/click.wav"));
        assert!(id.is_some(), "精确匹配应返回 ID");

        // 查询使用不匹配的路径
        let id_wrong = manager.find_by_path(Path::new("se/click.wav"));
        assert!(id_wrong.is_none(), "缺少 assets/ 前缀的路径不应匹配");

        let id_absolute = manager.find_by_path(&project.base_path.join("assets/se/click.wav"));
        assert!(
            id_absolute.is_none(),
            "绝对路径不应匹配（索引存储相对路径）"
        );
    }

    // ========================================================================
    // 其他测试
    // ========================================================================

    /// 验证 `resolve_path()` 正确拼接完整路径。
    #[test]
    fn test_resolve_path() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        let bg_id = manager
            .find_by_path(Path::new("assets/bg/background.png"))
            .expect("background.png 应在索引中");

        let full_path = manager
            .resolve_path(bg_id)
            .expect("resolve_path 应返回路径");
        assert!(full_path.is_absolute(), "解析路径应为绝对路径");
        assert!(
            full_path.ends_with("assets/bg/background.png"),
            "路径应以资源文件结尾"
        );
        assert!(full_path.exists(), "解析路径指向的文件应存在");
    }

    /// 验证 `get_metadata` 对无效 ID 返回 None。
    #[test]
    fn test_get_metadata_invalid_id() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);
        manager.scan_assets().expect("扫描应成功");

        assert!(manager.get_metadata(AssetId(0)).is_none());
        assert!(manager.get_metadata(AssetId(99999)).is_none());
    }

    /// 验证重复扫描不会产生重复条目（幂等性）。
    #[test]
    fn test_double_scan_is_idempotent() {
        let project = TestProject::new();
        let mut manager = AssetManager::new(&project.base_path);

        let count1 = manager.scan_assets().expect("第一次扫描应成功");
        let count2 = manager.scan_assets().expect("第二次扫描应成功");

        assert_eq!(count2, 0, "第二次扫描应返回 0（无新增文件）");
        assert_eq!(manager.asset_count(), count1, "资源总数不变");
    }

    /// 验证 `.aster_cache/` 目录中的文件被跳过。
    #[test]
    fn test_aster_cache_is_skipped() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");
        let assets_bg = dir.path().join("assets").join("bg");
        let cache_dir = dir.path().join("assets").join(".aster_cache");
        fs::create_dir_all(&assets_bg).expect("创建 bg 目录");
        fs::create_dir_all(&cache_dir).expect("创建 .aster_cache 目录");

        fs::write(assets_bg.join("valid.png"), b"data").expect("写入合法文件");
        fs::write(cache_dir.join("cached_file.png"), b"data").expect("写入缓存文件");

        let mut manager = AssetManager::new(dir.path());
        let count = manager.scan_assets().expect("扫描应成功");

        assert_eq!(count, 1, "只应索引 bg/ 中的文件，.aster_cache 中的应跳过");
    }

    /// 验证 `base_path()` 返回正确的根目录。
    #[test]
    fn test_base_path() {
        let manager = AssetManager::new(PathBuf::from("/test/project"));
        assert_eq!(manager.base_path(), Path::new("/test/project"));
    }

    /// 验证 AssetManager 的 new() 初始化正确的内部状态。
    #[test]
    fn test_new_asset_manager_is_empty() {
        let manager = AssetManager::new(PathBuf::from("/test"));
        assert_eq!(manager.asset_count(), 0);
        assert!(manager.assets().next().is_none());
    }
}
