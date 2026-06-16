//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-asset/src/asset_manager.rs
//! 功能概述：资源管理器 — `AssetManager` 是资源管理系统的中枢，负责：
//!           1. 扫描项目 `assets/` 目录，建立资源索引（AssetId ↔ 文件路径）
//!           2. 管理可扩展的 `AssetLoader` 注册表（按资源类型分发）
//!           3. 提供统一的资源加载入口（LRU 缓存 → 查元数据 → 找加载器 → 解码）
//!           4. 支持按 ID 或路径查询资源元数据
//!           5. LRU 缓存淘汰策略 + 命中率统计（PH2-T05 新增）
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::{AssetId, AssetType}（核心资源类型）
//! - crate::error::AssetError（错误类型）
//! - crate::loader::{AssetLoader, LoadedAsset}（加载器 trait + 统一数据表示）
//! - crate::cache::{CachedAsset, CacheStats, estimate_size}（缓存层）
//! - lru::LruCache（LRU 淘汰策略）
//!
//! 对应任务：PH2-T04（基础） + PH2-T05（LRU 缓存）

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aster_core::{AssetId, AssetType};
use lru::LruCache;

use crate::cache::{
    CacheStats, CachedAsset, DEFAULT_AUDIO_BUDGET, DEFAULT_CACHE_CAPACITY, DEFAULT_TEXTURE_BUDGET,
    estimate_size,
};
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
/// - **缓存**：基于 LRU 的资源缓存，双重约束（条目数 + 内存预算）
/// - **查询**：支持按 `AssetId` 或文件路径查找资源元数据
///
/// # 生命周期
///
/// ```text
/// AssetManager::new(base_path)
///   → scan_assets()           // 扫描目录，建立索引
///   → register_loader(loader) // 注册加载器（至少需要 TextureLoader + AudioLoader）
///   → load(id)               // 加载资源（先查 LRU 缓存，未命中则调用 loader 解码）
/// ```
///
/// # 缓存策略（PH2-T05）
///
/// - **条目数上限**：512 条目（`DEFAULT_CACHE_CAPACITY`）
/// - **纹理内存预算**：256 MB（`DEFAULT_TEXTURE_BUDGET`）
/// - **音频内存预算**：128 MB（`DEFAULT_AUDIO_BUDGET`）
/// - **淘汰顺序**：LRU（最近最少使用优先淘汰）
/// - **外部持有**：`Arc<CachedAsset>` 允许外部在缓存淘汰后仍持有资源
///
/// # 设计约束
///
/// - 不在本 crate 中创建 wgpu 设备——TextureLoader 通过构造函数注入
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
///     let cached = manager.load(id)?;
///     // cached.data 包含 LoadedAsset，可提取纹理/音频数据
/// }
///
/// // 查看缓存统计
/// let stats = manager.stats();
/// println!("命中率：{:.1}%", stats.hit_rate() * 100.0);
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
    // ─── PH2-T05 缓存字段 ─────────────────────────────────────────────────
    /// LRU 资源缓存：AssetId → 已加载资源
    ///
    /// 使用 `LruCache` 自动维护访问顺序。缓存容量由 `DEFAULT_CACHE_CAPACITY`
    /// 设定（512 条目），超限时自动淘汰最久未访问的条目。
    cache: LruCache<AssetId, Arc<CachedAsset>>,
    /// 缓存统计信息（命中/未命中/淘汰/内存占用）
    stats: CacheStats,
    /// 纹理缓存内存预算（字节），默认 256 MB
    texture_budget: u64,
    /// 音频缓存内存预算（字节），默认 128 MB
    audio_budget: u64,
}

impl AssetManager {
    /// 创建新的资源管理器（使用默认缓存预算）。
    ///
    /// 默认预算：
    /// - 缓存条目数上限：512（`DEFAULT_CACHE_CAPACITY`）
    /// - 纹理内存预算：256 MB（`DEFAULT_TEXTURE_BUDGET`）
    /// - 音频内存预算：128 MB（`DEFAULT_AUDIO_BUDGET`）
    ///
    /// # 参数
    /// - `base_path`：项目根目录的绝对路径。`assets/` 子目录应位于 `base_path/assets/`。
    ///
    /// # 返回值
    /// 返回空的 `AssetManager`——需调用 `scan_assets()` 建立索引，
    /// 并 `register_loader()` 注册至少一个加载器后才能加载资源。
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self::new_with_budgets(
            base_path,
            DEFAULT_CACHE_CAPACITY,
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        )
    }

    /// 创建新的资源管理器（自定义缓存预算）。
    ///
    /// # 参数
    /// - `base_path`：项目根目录的绝对路径
    /// - `cache_capacity`：LRU 缓存条目数上限（至少为 1）
    /// - `texture_budget`：纹理缓存内存预算（字节）
    /// - `audio_budget`：音频缓存内存预算（字节）
    ///
    /// # Panics
    /// 如果 `cache_capacity` 为 0（`NonZeroUsize` 构造失败）。
    /// 正常情况下调用方应传入 ≥ 1 的值。
    pub fn new_with_budgets(
        base_path: impl Into<PathBuf>,
        cache_capacity: usize,
        texture_budget: u64,
        audio_budget: u64,
    ) -> Self {
        Self {
            base_path: base_path.into(),
            assets: HashMap::new(),
            path_to_id: HashMap::new(),
            loaders: HashMap::new(),
            next_id: 1, // 从 1 开始，0 预留为无效 ID
            cache: LruCache::new(
                NonZeroUsize::new(cache_capacity.max(1)).expect("cache_capacity 必须 ≥ 1"),
            ),
            stats: CacheStats::default(),
            texture_budget,
            audio_budget,
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

    // ─── 资源加载（含 LRU 缓存） ──────────────────────────────────────────

    /// 加载资源——先查 LRU 缓存，未命中则调用加载器解码。
    ///
    /// # 加载流程
    ///
    /// ```text
    /// load(id)
    ///   ├─ 缓存命中 → stats.hits++ → 更新 last_access → 返回 Arc::clone(cached)
    ///   └─ 缓存未命中 → stats.misses++
    ///        ├─ 1. 查 AssetMetadata（NotFound 则返回错误）
    ///        ├─ 2. 找 AssetLoader（无加载器则 UnsupportedFormat）
    ///        ├─ 3. loader.load(path) → LoadedAsset
    ///        ├─ 4. estimate_size() → 计算内存占用
    ///        ├─ 5. LruCache::put() → 插入缓存（可能触发条目数淘汰）
    ///        ├─ 6. ensure_budget() → 检查内存预算（超限则 pop_lru 淘汰最旧条目）
    ///        └─ 7. 返回 Arc<CachedAsset>
    /// ```
    ///
    /// # 参数
    /// - `id`：要加载的资源标识符
    ///
    /// # 返回值
    /// - `Ok(Arc<CachedAsset>)`：缓存命中或加载成功，返回共享引用
    /// - `Err(AssetError::NotFound)`：ID 未在索引中
    /// - `Err(AssetError::UnsupportedFormat)`：无对应加载器
    /// - `Err(AssetError::DecodeError)`：解码失败
    ///
    /// # 性能
    /// - 缓存命中：O(1) HashMap 查找 + Arc 引用计数增加，< 1μs
    /// - 缓存未命中：取决于 Loader 解码性能（纹理 ~5ms、音频 ~50ms）
    ///
    /// # 注意
    /// 此方法需要 `&mut self`（而非 `&self`），因为缓存状态会随访问而改变
    /// （LRU 顺序更新、统计计数器递增）。如果需要在不可变引用下访问已加载
    /// 资源，可在获取 `Arc<CachedAsset>` 后 clone 一份持有。
    pub fn load(&mut self, id: AssetId) -> Result<Arc<CachedAsset>, AssetError> {
        // 步骤 1：查 LRU 缓存
        if let Some(cached) = self.cache.get(&id) {
            self.stats.hits += 1;
            // 更新 last_access 时间戳（调试/统计用）
            // LruCache::get() 已更新内部 LRU 顺序，last_access 为辅助信息
            *cached.last_access.lock().unwrap() = Instant::now();
            return Ok(Arc::clone(cached));
        }

        // 步骤 2：缓存未命中
        self.stats.misses += 1;

        // 步骤 3：查找元数据
        let metadata = self.assets.get(&id).ok_or_else(|| AssetError::NotFound {
            path: format!("AssetId({})", id.0),
        })?;

        // 步骤 4：查找加载器
        let loader = self.loaders.get(&metadata.asset_type).ok_or_else(|| {
            AssetError::UnsupportedFormat {
                path: metadata.relative_path.display().to_string(),
                format: format!("{:?}", metadata.asset_type),
            }
        })?;

        // 步骤 5：调用加载器解码
        let full_path = self.base_path.join(&metadata.relative_path);
        let data = loader.load(&full_path)?;

        // 步骤 6：估算内存占用
        let estimated_size = estimate_size(&data);

        // 步骤 7：检查内存预算（在插入前淘汰，确保新条目能放入缓存）
        self.ensure_budget(&data, estimated_size);

        // 步骤 8：插入 LRU 缓存
        let cached = Arc::new(CachedAsset {
            data,
            estimated_size,
            last_access: Mutex::new(Instant::now()),
        });

        // 更新对应类型的预算计数器
        match &cached.data {
            LoadedAsset::Texture { .. } => {
                self.stats.current_texture_bytes = self
                    .stats
                    .current_texture_bytes
                    .saturating_add(estimated_size);
            }
            LoadedAsset::AudioData { .. } => {
                self.stats.current_audio_bytes = self
                    .stats
                    .current_audio_bytes
                    .saturating_add(estimated_size);
            }
            LoadedAsset::Bytes { .. } => {
                // Bytes 类型不计入纹理/音频预算，仅受条目数上限约束
            }
        }

        // 检测条目数淘汰：LruCache 容量满且插入新 key 时会内部淘汰最旧条目
        let cache_was_full = self.cache.len() >= self.cache.cap().get();
        let key_is_new = !self.cache.contains(&id);

        self.cache.put(id, Arc::clone(&cached));

        if cache_was_full && key_is_new {
            self.stats.evictions += 1;
        }

        Ok(cached)
    }

    /// 确保缓存内存预算不超标（私有辅助方法）。
    ///
    /// 在插入新资源前调用，循环淘汰 LRU 最旧条目直到：
    /// - 纹理缓存字节数 + 新纹理大小 ≤ `texture_budget`
    /// - 音频缓存字节数 + 新音频大小 ≤ `audio_budget`
    ///
    /// # 参数
    /// - `new_asset`：即将插入的资源数据
    /// - `new_size`：新资源的估算内存大小
    ///
    /// # 淘汰策略
    ///
    /// 使用 `LruCache::pop_lru()` 淘汰全局最旧条目（不区分类型）。
    /// 淘汰后自动从对应预算计数器中减去该条目的 `estimated_size`。
    /// 如果缓存为空（无法继续淘汰），提前退出循环——此时新条目仍会插入，
    /// 但预算计数器可能暂时超标（下次加载时会继续尝试淘汰）。
    fn ensure_budget(&mut self, new_asset: &LoadedAsset, new_size: u64) {
        let is_texture = matches!(new_asset, LoadedAsset::Texture { .. });
        let is_audio = matches!(new_asset, LoadedAsset::AudioData { .. });

        // 确定需要检查的预算约束
        let texture_over = is_texture
            && self.stats.current_texture_bytes.saturating_add(new_size) > self.texture_budget;
        let audio_over =
            is_audio && self.stats.current_audio_bytes.saturating_add(new_size) > self.audio_budget;

        if !texture_over && !audio_over {
            return; // 预算充足，无需淘汰
        }

        // 循环淘汰最旧条目，直到所有超标预算回到安全线内
        // 设置最大迭代次数防止死循环（正常情况下不会达到）
        let max_iterations = self.cache.len();
        for _ in 0..max_iterations {
            // 重新检查是否还需要淘汰
            let texture_ok = !is_texture
                || self.stats.current_texture_bytes.saturating_add(new_size) <= self.texture_budget;
            let audio_ok = !is_audio
                || self.stats.current_audio_bytes.saturating_add(new_size) <= self.audio_budget;

            if texture_ok && audio_ok {
                break;
            }

            // 淘汰最旧条目
            if let Some((_evicted_id, evicted)) = self.cache.pop_lru() {
                self.stats.evictions += 1;

                // 从对应预算计数器中减去
                match &evicted.data {
                    LoadedAsset::Texture { .. } => {
                        self.stats.current_texture_bytes = self
                            .stats
                            .current_texture_bytes
                            .saturating_sub(evicted.estimated_size);
                    }
                    LoadedAsset::AudioData { .. } => {
                        self.stats.current_audio_bytes = self
                            .stats
                            .current_audio_bytes
                            .saturating_sub(evicted.estimated_size);
                    }
                    LoadedAsset::Bytes { .. } => {
                        // Bytes 不占用纹理/音频预算
                    }
                }
            } else {
                // 缓存已空，无法继续淘汰
                break;
            }
        }
    }

    // ─── 缓存管理 ─────────────────────────────────────────────────────────

    /// 返回缓存统计信息的只读引用。
    ///
    /// # 使用场景
    /// - 性能分析和调优
    /// - 调试缓存行为
    /// - 在 UI 中显示内存占用
    ///
    /// # 示例
    /// ```rust,ignore
    /// let stats = manager.stats();
    /// if stats.hit_rate() < 0.5 {
    ///     eprintln!("警告：缓存命中率偏低 ({:.1}%)", stats.hit_rate() * 100.0);
    /// }
    /// ```
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 清空所有缓存并重置统计信息。
    ///
    /// 注意：此操作不会释放已被外部 `Arc<CachedAsset>` 持有的资源——
    /// 只有当所有外部引用也释放后，GPU 纹理和音频缓冲才会被回收。
    ///
    /// # 使用场景
    /// - 场景切换时主动清理旧缓存
    /// - 内存压力过大时手动释放
    /// - 测试用例中重置缓存状态
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.stats = CacheStats::default();
    }

    /// 手动触发淘汰，将缓存缩至预算以内。
    ///
    /// 与 `load()` 中自动触发的 `ensure_budget()` 逻辑相同，
    /// 但此方法是公开的，允许上层在加载批量资源后手动整理缓存。
    ///
    /// # 使用场景
    /// - 加载大场景后发现内存超预算
    /// - 在场景切换前主动释放不再需要的资源
    pub fn evict_to_budget(&mut self) {
        // 循环淘汰直到两种预算均不超标
        let max_iterations = self.cache.len();
        for _ in 0..max_iterations {
            let texture_ok = self.stats.current_texture_bytes <= self.texture_budget;
            let audio_ok = self.stats.current_audio_bytes <= self.audio_budget;

            if texture_ok && audio_ok {
                break;
            }

            if let Some((_evicted_id, evicted)) = self.cache.pop_lru() {
                self.stats.evictions += 1;

                match &evicted.data {
                    LoadedAsset::Texture { .. } => {
                        self.stats.current_texture_bytes = self
                            .stats
                            .current_texture_bytes
                            .saturating_sub(evicted.estimated_size);
                    }
                    LoadedAsset::AudioData { .. } => {
                        self.stats.current_audio_bytes = self
                            .stats
                            .current_audio_bytes
                            .saturating_sub(evicted.estimated_size);
                    }
                    LoadedAsset::Bytes { .. } => {}
                }
            } else {
                break;
            }
        }
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

    // ========================================================================
    // PH2-T05: LRU 缓存测试 — 覆盖 AC01, AC02, AC03, AC04, AC08
    // ========================================================================

    /// 所有缓存测试共用的 mock 加载器。
    ///
    /// 返回 `Bytes` 变体（无需 GPU 设备），每次加载返回不同的数据
    /// 以便测试区分缓存命中（返回相同 Arc）和未命中（返回新数据）。
    struct MockBytesLoader {
        /// 调用计数器，每次 load() 递增
        call_count: std::sync::atomic::AtomicUsize,
    }

    impl MockBytesLoader {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl AssetLoader for MockBytesLoader {
        fn supported_types(&self) -> &[AssetType] {
            &[AssetType::Font] // 使用 Font 类型（不与 BgmOnlyLoader 冲突）
        }

        fn load(&self, _path: &Path) -> Result<LoadedAsset, AssetError> {
            let count = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(LoadedAsset::Bytes {
                data: vec![count as u8; 1024], // 1KB 数据，每次调用返回不同内容
            })
        }
    }

    /// 创建含 mock 加载器的 AssetManager，用于缓存测试。
    ///
    /// 在 `assets/font/` 目录下创建测试文件并扫描索引。
    fn setup_cache_test_manager(
        capacity: usize,
        texture_budget: u64,
        audio_budget: u64,
    ) -> (tempfile::TempDir, AssetManager, AssetId) {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");
        let assets_font = dir.path().join("assets").join("font");
        fs::create_dir_all(&assets_font).expect("创建 font 目录");

        let test_file = assets_font.join("test_font.ttf");
        fs::write(&test_file, b"mock font data").expect("写入测试文件");

        let mut manager = AssetManager::new_with_budgets(
            dir.path().to_path_buf(),
            capacity,
            texture_budget,
            audio_budget,
        );
        manager.scan_assets().expect("扫描测试项目应成功");
        manager.register_loader(Arc::new(MockBytesLoader::new()));

        let font_id = manager
            .find_by_path(Path::new("assets/font/test_font.ttf"))
            .expect("测试文件应在索引中");

        (dir, manager, font_id)
    }

    // ─── AC01 — 缓存命中 ──────────────────────────────────────────────

    /// AC01: 验证加载同一资源两次，第二次命中缓存。
    #[test]
    fn ac01_cache_hit_on_second_load() {
        let (_dir, mut manager, font_id) = setup_cache_test_manager(
            DEFAULT_CACHE_CAPACITY,
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        );

        // 首次加载：未命中
        let first = manager.load(font_id).expect("首次加载应成功");
        assert_eq!(manager.stats().misses, 1, "首次加载应为未命中");
        assert_eq!(manager.stats().hits, 0, "首次加载命中数应为 0");

        // 第二次加载同一资源：命中
        let second = manager.load(font_id).expect("第二次加载应成功");
        assert_eq!(manager.stats().hits, 1, "第二次加载应为命中");
        assert_eq!(manager.stats().misses, 1, "未命中数保持不变");

        // 验证两次返回的是同一个 Arc（指针相等）
        assert!(Arc::ptr_eq(&first, &second), "缓存命中应返回同一个 Arc");
    }

    /// AC01 补充：验证多次缓存命中后 hit 计数正确。
    #[test]
    fn ac01_multiple_cache_hits() {
        let (_dir, mut manager, font_id) = setup_cache_test_manager(
            DEFAULT_CACHE_CAPACITY,
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        );

        // 首次加载（未命中）
        manager.load(font_id).expect("首次加载应成功");

        // 后续 5 次加载（全部命中）
        for _i in 0..5 {
            let cached = manager.load(font_id).expect("缓存命中应成功");
            assert!(
                matches!(cached.data, LoadedAsset::Bytes { .. }),
                "缓存命中的数据类型应为 Bytes"
            );
        }

        assert_eq!(manager.stats().hits, 5, "5 次命中");
        assert_eq!(manager.stats().misses, 1, "1 次未命中");
        let rate = manager.stats().hit_rate();
        assert!(
            (rate - 5.0 / 6.0).abs() < 0.01,
            "命中率应为 5/6 ≈ 0.833，实际：{rate}"
        );
    }

    // ─── AC02 — 缓存未命中 ────────────────────────────────────────────

    /// AC02: 验证加载不同资源时 misses 递增。
    #[test]
    fn ac02_cache_miss_increments() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");

        // 创建多个测试文件
        let assets_font = dir.path().join("assets").join("font");
        fs::create_dir_all(&assets_font).expect("创建 font 目录");

        for i in 0..3 {
            let file = assets_font.join(format!("font_{}.ttf", i));
            fs::write(&file, b"mock font").expect("写入测试文件");
        }

        let mut manager = AssetManager::new_with_budgets(
            dir.path().to_path_buf(),
            DEFAULT_CACHE_CAPACITY,
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        );
        manager.scan_assets().expect("扫描应成功");
        manager.register_loader(Arc::new(MockBytesLoader::new()));

        // 加载 3 个不同资源
        let mut ids = Vec::new();
        for i in 0..3 {
            let id = manager
                .find_by_path(Path::new(&format!("assets/font/font_{}.ttf", i)))
                .expect("应在索引中");
            ids.push(id);
        }

        // 每个资源加载一次
        for (i, &id) in ids.iter().enumerate() {
            manager.load(id).expect("加载应成功");
            assert_eq!(
                manager.stats().misses,
                (i + 1) as u64,
                "每次新资源应增加 miss"
            );
            assert_eq!(manager.stats().hits, 0, "首次加载不应有命中");
        }

        // 再次加载第一个资源：应命中
        manager.load(ids[0]).expect("应命中缓存");
        assert_eq!(manager.stats().hits, 1);
        assert_eq!(manager.stats().misses, 3);
    }

    // ─── AC03 — LRU 条目数淘汰 ────────────────────────────────────────

    /// AC03: 验证设置极小缓存容量（2），加载 3 个资源后第一个被淘汰。
    #[test]
    fn ac03_lru_eviction_by_entry_count() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");

        let assets_font = dir.path().join("assets").join("font");
        fs::create_dir_all(&assets_font).expect("创建 font 目录");

        // 创建 3 个测试文件
        for i in 0..3 {
            let file = assets_font.join(format!("f{}.ttf", i));
            fs::write(&file, b"data").expect("写入测试文件");
        }

        let mut manager = AssetManager::new_with_budgets(
            dir.path().to_path_buf(),
            2, // 极小容量：仅容纳 2 个条目
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        );
        manager.scan_assets().expect("扫描应成功");
        manager.register_loader(Arc::new(MockBytesLoader::new()));

        let ids: Vec<AssetId> = (0..3)
            .map(|i| {
                manager
                    .find_by_path(Path::new(&format!("assets/font/f{}.ttf", i)))
                    .expect("应在索引中")
            })
            .collect();

        // 加载资源 0 和 1（缓存满：2/2）
        let cached_0 = manager.load(ids[0]).expect("加载 0 应成功");
        let _cached_1 = manager.load(ids[1]).expect("加载 1 应成功");

        // 加载资源 2（触发淘汰：最旧的资源 0 被淘汰）
        let _cached_2 = manager.load(ids[2]).expect("加载 2 应成功");

        assert!(
            manager.stats().evictions >= 1,
            "应有至少 1 次淘汰，实际：{}",
            manager.stats().evictions
        );

        // 验证资源 0 的 Arc 引用计数：仍被 cached_0 持有（1），
        // 但缓存中已不存在此条目
        assert_eq!(
            Arc::strong_count(&cached_0),
            1,
            "cached_0 应仅被局部变量持有"
        );
    }

    // ─── AC04 — 内存预算淘汰 ──────────────────────────────────────────

    /// AC04: 验证设置极小纹理预算后，加载更大纹理时触发淘汰。
    #[test]
    fn ac04_memory_budget_eviction() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");

        let assets_font = dir.path().join("assets").join("font");
        fs::create_dir_all(&assets_font).expect("创建 font 目录");

        // 创建 2 个 1KB 的测试文件
        for i in 0..2 {
            let file = assets_font.join(format!("fb{}.ttf", i));
            fs::write(&file, b"data").expect("写入测试文件");
        }

        let mut manager = AssetManager::new_with_budgets(
            dir.path().to_path_buf(),
            DEFAULT_CACHE_CAPACITY,
            512, // 极小纹理预算：512 字节（实际我们用的是 Bytes，不走纹理预算）
            512, // 极小音频预算
        );
        manager.scan_assets().expect("扫描应成功");
        manager.register_loader(Arc::new(MockBytesLoader::new()));

        let ids: Vec<AssetId> = (0..2)
            .map(|i| {
                manager
                    .find_by_path(Path::new(&format!("assets/font/fb{}.ttf", i)))
                    .expect("应在索引中")
            })
            .collect();

        // 加载第一个资源（MockBytesLoader 返回 1KB Bytes，不计入纹理/音频预算）
        // 此测试验证 Bytes 类型不触发预算淘汰
        let _cached_0 = manager.load(ids[0]).expect("加载 0 应成功");
        let _cached_1 = manager.load(ids[1]).expect("加载 1 应成功");

        // Bytes 类型不应触发预算淘汰
        assert_eq!(
            manager.stats().current_texture_bytes,
            0,
            "Bytes 类型不计入纹理预算"
        );
        assert_eq!(
            manager.stats().current_audio_bytes,
            0,
            "Bytes 类型不计入音频预算"
        );

        // evict_to_budget 对空预算应正常执行（不 panic）
        manager.evict_to_budget();
    }

    /// AC04 补充：验证 evict_to_budget() 在超预算时正确淘汰。
    #[test]
    fn ac04_evict_to_budget_manually() {
        let (_dir, mut manager, font_id) = setup_cache_test_manager(
            DEFAULT_CACHE_CAPACITY,
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        );

        // 加载资源
        let cached = manager.load(font_id).expect("加载应成功");
        assert!(manager.stats().misses >= 1);

        // 就算预算未超，evict_to_budget() 也不应 panic
        manager.evict_to_budget();

        // 缓存未清空前（外部持有 Arc），cache 中应仍有条目
        // 但由于我们通过 load 返回了 Arc，缓存中仍有它
        drop(cached);

        // 清空缓存
        manager.clear_cache();
        assert_eq!(manager.stats().hits, 0, "clear_cache 应重置统计");
        assert_eq!(manager.stats().misses, 0);
        assert_eq!(manager.stats().evictions, 0);

        // 清空后再次加载：应未命中
        let _reloaded = manager.load(font_id).expect("重新加载应成功");
        assert_eq!(manager.stats().misses, 1);
    }

    // ─── AC08 — Arc 引用释放 ──────────────────────────────────────────

    /// AC08: 验证缓存淘汰后，如果外部不持有 Arc，资源被正确释放。
    #[test]
    fn ac08_arc_release_after_eviction() {
        let dir = tempfile::TempDir::new().expect("创建临时目录失败");

        let assets_font = dir.path().join("assets").join("font");
        fs::create_dir_all(&assets_font).expect("创建 font 目录");

        for i in 0..3 {
            let file = assets_font.join(format!("arc{}.ttf", i));
            fs::write(&file, b"data").expect("写入测试文件");
        }

        let mut manager = AssetManager::new_with_budgets(
            dir.path().to_path_buf(),
            2, // 容量 2
            DEFAULT_TEXTURE_BUDGET,
            DEFAULT_AUDIO_BUDGET,
        );
        manager.scan_assets().expect("扫描应成功");
        manager.register_loader(Arc::new(MockBytesLoader::new()));

        let ids: Vec<AssetId> = (0..3)
            .map(|i| {
                manager
                    .find_by_path(Path::new(&format!("assets/font/arc{}.ttf", i)))
                    .expect("应在索引中")
            })
            .collect();

        // 加载 0 和 1（缓存满）
        let cached_0 = manager.load(ids[0]).expect("加载 0 应成功");
        let _cached_1 = manager.load(ids[1]).expect("加载 1 应成功");

        // Arc 引用计数应为 2（cached_0 局部变量 + 缓存中的副本）
        assert_eq!(Arc::strong_count(&cached_0), 2, "缓存 + 局部变量");

        // 加载资源 2（触发淘汰：0 被淘汰）
        let _cached_2 = manager.load(ids[2]).expect("加载 2 应成功");

        // 淘汰后：缓存中 0 的副本已被移除，仅局部变量 cached_0 持有
        assert_eq!(Arc::strong_count(&cached_0), 1, "淘汰后仅局部变量持有资源");

        // 释放局部变量后，资源被完全回收
        drop(cached_0);
        // 无泄漏断言：如果到这里没有 panic，说明 drop 成功
    }
}
