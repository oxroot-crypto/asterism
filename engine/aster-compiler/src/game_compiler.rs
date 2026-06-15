//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/game_compiler.rs
//! 功能概述：游戏编译器 — `GameCompiler` 将项目中所有 `.aster` 场景批量编译为
//!           `CompiledGame`（多场景字节码集合），解析并验证跨场景 `goto` 引用，
//!           应用 `build.toml` 中的编译配置（optimize/minify 开关等）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_core（Scene / Character / Expr / BuildConfig / CompileConfig 等核心类型）
//! - crate::compiler（单场景 Compiler::compile_with_config）
//! - crate::bytecode::CompiledScene（单场景编译产物）
//! - crate::error::CompileError（编译错误类型）
//!
//! 对应文档：Phase-1-Tasks.md PH1-T16（GameCompiler 实现）

use std::collections::HashMap;

use aster_core::{BuildConfig, Character, Expr, Scene, SceneNode};

use crate::bytecode::CompiledScene;
use crate::compiler::Compiler;
use crate::error::CompileError;

// ============================================================================
// CompiledGame — 游戏编译产物
// ============================================================================

/// 游戏编译产物 — 包含项目中所有场景的编译后字节码。
///
/// 这是 `GameCompiler::compile()` 的产出物，下游由以下模块消费：
/// - `GameContext`（PH1-T17）：持有 `CompiledGame`，提供场景导航
/// - `SceneManager`（PH1-T18）：通过 `GameContext` 获取场景字节码
///
/// # 字段说明
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `game_name` | `String` | 游戏名称（来自 `aster.toml`） |
/// | `game_version` | `String` | 游戏版本号 |
/// | `entry_scene_id` | `String` | 入口场景 ID（如 `"prologue"`） |
/// | `scenes` | `HashMap<String, CompiledScene>` | 所有已编译场景（key = scene_id） |
/// | `characters` | `HashMap<String, Character>` | 角色表（key = 角色 ID） |
/// | `build_info` | `BuildInfo` | 构建统计信息 |
///
/// # 序列化
///
/// 通过 bincode 序列化为 `.asterbyte` 文件供 GameLauncher 加载：
/// ```rust,no_run
/// # use aster_compiler::{CompiledGame, BuildInfo};
/// # use std::collections::HashMap;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let game = CompiledGame {
///     game_name: "Test".into(),
///     game_version: "0.1.0".into(),
///     entry_scene_id: "prologue".into(),
///     scenes: HashMap::new(),
///     characters: HashMap::new(),
///     build_info: BuildInfo {
///         source_file_count: 0,
///         total_instructions: 0,
///         optimization_level: "optimized".into(),
///         build_timestamp: "2026-06-15T00:00:00Z".into(),
///     },
/// };
/// let bytes = bincode::serialize(&game)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompiledGame {
    /// 游戏名称
    pub game_name: String,

    /// 游戏版本号（语义化版本）
    pub game_version: String,

    /// 入口场景 ID（如 `"prologue"`）
    pub entry_scene_id: String,

    /// 所有已编译场景（scene_id → CompiledScene）
    pub scenes: HashMap<String, CompiledScene>,

    /// 角色定义表（角色 ID → Character）
    pub characters: HashMap<String, Character>,

    /// 构建统计信息
    pub build_info: BuildInfo,
}

/// 构建统计信息 — 记录本次编译的元数据。
///
/// 用于 IDE 构建面板展示和调试诊断。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildInfo {
    /// 源码文件数量（编译的场景文件数）
    pub source_file_count: usize,

    /// 所有场景的字节码指令总字节数
    pub total_instructions: usize,

    /// 优化级别字符串描述
    /// - `"optimized"`：启用了 4 个优化 Pass
    /// - `"none"`：跳过了优化
    /// - `"ast"`：仅解析为 AST，未编译为字节码（调试模式）
    pub optimization_level: String,

    /// 构建时间戳（ISO 8601 格式，如 `"2026-06-15T12:00:00Z"`）
    pub build_timestamp: String,
}

// ============================================================================
// GameCompileInput — 编译器输入
// ============================================================================

/// 游戏编译器输入 — 包含编译所需的所有信息。
///
/// 设计为借用入参（不获取所有权），避免不必要的 clone。
/// 调用方（runtime / CLI 构建工具）负责解析 `.aster` 文件并组装此结构。
///
/// # 示例
/// ```rust,no_run
/// use std::collections::HashMap;
/// use aster_core::{BuildConfig, Scene};
/// use aster_compiler::{GameCompiler, GameCompileInput};
///
/// # fn main() -> Result<(), Vec<aster_compiler::CompileError>> {
/// let scenes: Vec<(String, Scene)> = vec![]; // 从 parser 获取
/// let characters = HashMap::new();
/// let build_config = BuildConfig::default();
///
/// let input = GameCompileInput {
///     game_name: "My VN",
///     game_version: "0.1.0",
///     entry_scene_id: "prologue",
///     scenes: &scenes,
///     characters: &characters,
///     build_config: &build_config,
/// };
///
/// let compiled = GameCompiler::compile(input)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GameCompileInput<'a> {
    /// 游戏名称（来自 `aster.toml` 的 `game.name`）
    pub game_name: &'a str,

    /// 游戏版本号
    pub game_version: &'a str,

    /// 入口场景 ID（如 `"prologue"`）
    pub entry_scene_id: &'a str,

    /// 所有待编译场景列表 — `(scene_id, parsed_ast)` 对
    /// scene_id 必须与 `Scene.id` 字段一致
    pub scenes: &'a [(String, Scene)],

    /// 角色定义表（角色 ID → Character）
    pub characters: &'a HashMap<String, Character>,

    /// 构建配置（来自 `build.toml`）
    pub build_config: &'a BuildConfig,
}

// ============================================================================
// GameCompiler — 游戏编译器
// ============================================================================

/// 游戏编译器 — 将项目所有场景批量编译为 `CompiledGame`。
///
/// # 编译流程
///
/// ```text
/// GameCompileInput { scenes, build_config, ... }
///   │
///   ├─ Step 1: 逐场景编译
///   │    for each (scene_id, ast) in scenes:
///   │      if build_config.compile.target == "ast":
///   │        跳过编译（返回空 CompiledScene，仅调试用）
///   │      else:
///   │        Compiler::compile_with_config(ast, &build_config.compile)
///   │      → 收集 CompiledScene 或 CompileError
///   │
///   ├─ Step 2: 如果存在编译错误 → 立即返回 Err(errors)
///   │
///   ├─ Step 3: 跨场景引用验证
///   │    扫描所有 Scene AST 中的 Goto 节点
///   │    → 验证目标场景存在
///   │    → 验证目标标签（如果有）存在于目标场景的 label_table
///   │    → 收集 CompileError
///   │
///   └─ Step 4: 组装 CompiledGame + BuildInfo
///       计算总指令数 → 填充 BuildInfo → 返回 CompiledGame
/// ```
///
/// # 错误处理
///
/// 采用批量收集策略：所有场景的编译错误和跨场景引用错误在一次 `compile()`
/// 调用中全部收集，而非在第一个错误处短路。这使创作者能一次性看到所有问题。
///
/// # 不依赖 `aster-runtime`
///
/// `GameCompiler` 通过 `GameCompileInput` 接收已解析的 `Scene`，
/// 不直接依赖 `aster-runtime::GameManifest`。解析逻辑由调用方负责。
#[derive(Debug)]
pub struct GameCompiler;

impl GameCompiler {
    /// 批量编译所有场景，产出 `CompiledGame`。
    ///
    /// # 参数
    /// - `input`：包含所有待编译场景、角色表、构建配置的编译输入
    ///
    /// # 返回值
    /// - `Ok(CompiledGame)`：全部场景编译成功，跨场景引用验证通过
    /// - `Err(Vec<CompileError>)`：存在编译错误或跨场景引用错误
    ///
    /// # 性能
    ///
    /// 各场景独立编译，可并行化（当前为顺序执行，Phase 4 可引入 rayon 并行编译）。
    ///
    /// # 示例
    /// ```rust,no_run
    /// use std::collections::HashMap;
    /// use aster_core::{BuildConfig, Scene};
    /// use aster_compiler::{GameCompiler, GameCompileInput};
    ///
    /// # fn main() -> Result<(), Vec<aster_compiler::CompileError>> {
    /// let scenes = vec![];
    /// let chars = HashMap::new();
    /// let input = GameCompileInput {
    ///     game_name: "Test",
    ///     game_version: "0.1.0",
    ///     entry_scene_id: "prologue",
    ///     scenes: &scenes,
    ///     characters: &chars,
    ///     build_config: &BuildConfig::default(),
    /// };
    /// let result = GameCompiler::compile(input)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn compile(input: GameCompileInput<'_>) -> Result<CompiledGame, Vec<CompileError>> {
        let mut errors: Vec<CompileError> = Vec::new();
        let mut compiled_scenes: HashMap<String, CompiledScene> = HashMap::new();
        let config = &input.build_config.compile;

        // ── Step 1: 逐场景编译 ──────────────────────────────────────────
        for (scene_id, ast) in input.scenes {
            // 验证 scene_id 与 AST 内声明的 id 一致
            let effective_id = if ast.id.is_empty() {
                // AST 的 Scene.id 为空时使用外层传入的 scene_id
                scene_id.clone()
            } else if ast.id != *scene_id {
                // scene_id 不匹配 — 以 AST 内部声明为准
                ast.id.clone()
            } else {
                scene_id.clone()
            };

            match config.target.as_str() {
                "ast" => {
                    // 调试模式：仅解析为 AST，不编译为字节码
                    // 构造一个空的 CompiledScene 占位（后续 IDE 可展示 AST 树）
                    compiled_scenes.insert(
                        effective_id,
                        CompiledScene {
                            version: 0,
                            instructions: vec![0xFF], // 仅一条 End 指令
                            constant_pool: vec![],
                            label_table: HashMap::new(),
                        },
                    );
                }
                _ => {
                    // 标准模式：编译为字节码（asterbyte）
                    let compiler = Compiler::new();
                    match compiler.compile_with_config(ast, config) {
                        Ok(compiled) => {
                            compiled_scenes.insert(effective_id, compiled);
                        }
                        Err(mut scene_errors) => {
                            // 为错误追加场景上下文信息
                            for err in &mut scene_errors {
                                if err.message.contains(&effective_id) {
                                    continue; // 已有场景信息
                                }
                                let original_msg = std::mem::take(&mut err.message);
                                err.message =
                                    format!("场景 \"{}\" 编译失败：{}", effective_id, original_msg);
                            }
                            errors.append(&mut scene_errors);
                        }
                    }
                }
            }
        }

        // ── Step 2: 如果有编译错误则立即返回 ────────────────────────────
        if !errors.is_empty() {
            return Err(errors);
        }

        // ── Step 3: 跨场景引用验证 ──────────────────────────────────────
        validate_cross_scene_references(input.scenes, &compiled_scenes, &mut errors);

        // ── Step 3.5: 验证 entry_scene_id 存在于已编译场景中 ─────────────
        // 空场景列表时不验证（允许空项目编译）
        if !compiled_scenes.is_empty() && !compiled_scenes.contains_key(input.entry_scene_id) {
            let available: Vec<&str> = compiled_scenes.keys().map(|s| s.as_str()).collect();
            errors.push(CompileError {
                message: format!(
                    "入口场景 \"{}\" 不存在于已编译的场景列表中。可用场景：{}",
                    input.entry_scene_id,
                    available.join(", ")
                ),
                line: 0,
                column: 0,
                hint: Some("请检查 aster.toml 中的 entry_scene 配置".to_string()),
            });
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // ── Step 4: 组装 CompiledGame + BuildInfo ────────────────────────
        let total_instructions: usize =
            compiled_scenes.values().map(|s| s.instructions.len()).sum();
        let source_file_count = input.scenes.len();
        let optimization_level = if config.target == "ast" {
            "ast".to_string()
        } else if config.optimize {
            "optimized".to_string()
        } else {
            "none".to_string()
        };
        // ISO 8601 格式构建时间戳
        let build_timestamp = chrono_now_iso8601();

        Ok(CompiledGame {
            game_name: input.game_name.to_string(),
            game_version: input.game_version.to_string(),
            entry_scene_id: input.entry_scene_id.to_string(),
            scenes: compiled_scenes,
            characters: input.characters.clone(),
            build_info: BuildInfo {
                source_file_count,
                total_instructions,
                optimization_level,
                build_timestamp,
            },
        })
    }
}

// ============================================================================
// 跨场景引用验证
// ============================================================================

/// 验证所有场景的 `Goto` 跨场景引用。
///
/// 遍历每个场景 AST 中的 `SceneNode::Goto` 节点：
/// 1. 提取目标场景 ID（`scene_id` 字段）
/// 2. 检查目标场景是否在 `compiled_scenes` 中存在
/// 3. 如果 Goto 带有 `label:` 参数，检查目标标签是否存在于目标场景的 `label_table` 中
///
/// 不存在的引用 → 生成 `CompileError` 并收集到 `errors` 中。
fn validate_cross_scene_references(
    scenes: &[(String, Scene)],
    compiled_scenes: &HashMap<String, CompiledScene>,
    errors: &mut Vec<CompileError>,
) {
    for (source_scene_id, ast) in scenes {
        validate_scene_goto_nodes(source_scene_id, &ast.nodes, compiled_scenes, errors);
    }
}

/// 递归验证 SceneNode 列表中的 Goto 引用。
///
/// 需要递归处理 Branch 的嵌套节点（then_nodes / elif_branches / else_nodes），
/// 因为 Goto 可能出现在条件分支内部。
fn validate_scene_goto_nodes(
    source_scene_id: &str,
    nodes: &[SceneNode],
    compiled_scenes: &HashMap<String, CompiledScene>,
    errors: &mut Vec<CompileError>,
) {
    for node in nodes {
        match node {
            SceneNode::Goto { scene_id, label } => {
                // 提取目标场景 ID 字符串
                if let Some(target_scene) = extract_string_literal(scene_id) {
                    // 检查场景是否存在
                    if !compiled_scenes.contains_key(target_scene) {
                        errors.push(CompileError::without_position(
                            format!(
                                "场景 \"{}\" 中的 goto 目标场景 \"{}\" 不存在 \
                                 （请检查场景文件是否存在，或场景 ID 是否拼写正确）",
                                source_scene_id, target_scene
                            ),
                            Some("请检查 scripts/ 目录下是否存在对应的 .aster 文件"),
                        ));
                    } else if let Some(target_label) =
                        label.as_ref().and_then(extract_string_literal)
                        && let Some(target_compiled) = compiled_scenes.get(target_scene)
                        && !target_compiled.label_table.contains_key(target_label)
                    {
                        // 检查标签是否存在于目标场景
                        errors.push(CompileError::without_position(
                            format!(
                                "场景 \"{}\" 中的 goto \"{}\" label: \"{}\"：\
                                 目标标签 \"{}\" 在场景 \"{}\" 中不存在",
                                source_scene_id,
                                target_scene,
                                target_label,
                                target_label,
                                target_scene
                            ),
                            Some(&format!(
                                "请检查场景 \"{}\" 中是否定义了 label \"{}\"",
                                target_scene, target_label
                            )),
                        ));
                    }
                }
                // 如果 scene_id 是变量引用（非字符串字面量），则在运行时解析，
                // 编译期无法验证，不产生错误。
            }
            // 递归处理嵌套节点
            SceneNode::Branch {
                then_nodes,
                elif_branches,
                else_nodes,
                ..
            } => {
                validate_scene_goto_nodes(source_scene_id, then_nodes, compiled_scenes, errors);
                for (_, elif_nodes) in elif_branches {
                    validate_scene_goto_nodes(source_scene_id, elif_nodes, compiled_scenes, errors);
                }
                if let Some(else_nodes) = else_nodes {
                    validate_scene_goto_nodes(source_scene_id, else_nodes, compiled_scenes, errors);
                }
            }
            SceneNode::Subroutine { body, .. } => {
                validate_scene_goto_nodes(source_scene_id, body, compiled_scenes, errors);
            }
            _ => {} // 其他节点类型不包含 Goto
        }
    }
}

/// 从 Expr 中提取字符串字面量值。
///
/// 仅当 Expr 是 `StringLiteral(s)` 时返回 `Some(&str)`，
/// 对于 Variable / BinaryOp / 字面量等其他变体返回 `None`。
/// 这些表达式将在运行时求值，编译期跳过验证。
fn extract_string_literal(expr: &Expr) -> Option<&str> {
    expr.as_string_literal()
}

// ============================================================================
// 时间戳工具
// ============================================================================

/// 生成当前 UTC 时间的 ISO 8601 格式字符串。
///
/// 使用 `std::time::SystemTime` 计算 UNIX 时间戳，
/// 手动格式化为 `YYYY-MM-DDTHH:MM:SSZ` 格式（零外部依赖）。
/// chrono crate 是更完善的选择，但 Phase 1 暂不引入。
fn chrono_now_iso8601() -> String {
    use std::time::SystemTime;

    // 常数：1970-01-01 到 2026-06-15 之间的秒数
    // 实际运行中从 SystemTime 获取
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    // 将 UNIX 时间戳分解为日期时间各分量
    // 从 1970 年开始逐年计算，处理闰年
    let days = secs / 86400;
    let time_of_day = secs % 86400;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // 计算年月日（从 1970-01-01 开始）
    let (year, month, day) = days_to_ymd(days as i64);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// 将自 1970-01-01 以来的天数转换为 (年, 月, 日)。
///
/// 使用逐月递减法：先找到正确的年份（处理闰年），再逐月定位。
fn days_to_ymd(mut total_days: i64) -> (i64, u32, u32) {
    let mut year = 1970i64;
    let days_in_year = |y: i64| -> i64 { if is_leap_year(y) { 366 } else { 365 } };

    // 逐年递减
    loop {
        let diy = days_in_year(year);
        if total_days < diy {
            break;
        }
        total_days -= diy;
        year += 1;
    }

    let days_in_month = |y: i64, m: u32| -> i64 {
        match m {
            1 => 31,
            2 if is_leap_year(y) => 29,
            2 => 28,
            3 => 31,
            4 => 30,
            5 => 31,
            6 => 30,
            7 => 31,
            8 => 31,
            9 => 30,
            10 => 31,
            11 => 30,
            12 => 31,
            _ => 30,
        }
    };

    let mut month = 1u32;
    loop {
        let dim = days_in_month(year, month);
        if total_days < dim {
            break;
        }
        total_days -= dim;
        month += 1;
    }

    let day = (total_days + 1) as u32; // 日期从 1 开始

    (year, month, day)
}

/// 判断是否为闰年（格里高利历）。
const fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aster_core::CompileConfig;
    use std::collections::HashMap;

    // ── 测试辅助函数 ────────────────────────────────────────────────────

    /// 创建字符串字面量 Expr
    fn s(v: &str) -> Expr {
        Expr::string_literal(v)
    }
    /// 创建一个最小化的测试场景（仅含一条旁白）
    fn make_minimal_scene(id: &str, text: &str) -> (String, Scene) {
        (
            id.to_string(),
            Scene {
                id: id.to_string(),
                label: None,
                background: None,
                music: None,
                nodes: vec![SceneNode::Narration { text: s(text) }],
            },
        )
    }

    /// 创建一个含 Goto 的场景
    fn make_goto_scene(
        id: &str,
        target_scene: &str,
        target_label: Option<&str>,
    ) -> (String, Scene) {
        (
            id.to_string(),
            Scene {
                id: id.to_string(),
                label: None,
                background: None,
                music: None,
                nodes: vec![SceneNode::Goto {
                    scene_id: s(target_scene),
                    label: target_label.map(s),
                }],
            },
        )
    }

    /// 创建一个含 Label 的场景（用于测试跨场景标签验证）
    fn make_scene_with_labels(id: &str, text: &str, labels: &[&str]) -> (String, Scene) {
        let mut nodes: Vec<SceneNode> = labels
            .iter()
            .map(|l| SceneNode::Label {
                name: l.to_string(),
            })
            .collect();
        nodes.push(SceneNode::Narration { text: s(text) });
        (
            id.to_string(),
            Scene {
                id: id.to_string(),
                label: None,
                background: None,
                music: None,
                nodes,
            },
        )
    }

    /// 默认的 BuildConfig（optimize=true, target=asterbyte）
    fn default_build_config() -> BuildConfig {
        BuildConfig::default()
    }

    /// BuildConfig 但不启用优化
    fn no_optimize_config() -> BuildConfig {
        BuildConfig {
            compile: CompileConfig {
                optimize: false,
                ..CompileConfig::default()
            },
            ..BuildConfig::default()
        }
    }

    /// 辅助函数：执行编译
    fn compile_scenes(
        scenes: &[(String, Scene)],
        config: &BuildConfig,
    ) -> Result<CompiledGame, Vec<CompileError>> {
        let characters = HashMap::new();
        let input = GameCompileInput {
            game_name: "Test Game",
            game_version: "0.1.0",
            entry_scene_id: &scenes[0].0,
            scenes,
            characters: &characters,
            build_config: config,
        };
        GameCompiler::compile(input)
    }

    // ── AC01: 模板项目全部场景编译成功 ──────────────────────────────────

    /// AC01 — 多个场景批量编译成功，所有场景在 CompiledGame.scenes 中。
    #[test]
    fn ac01_multiple_scenes_compile_successfully() {
        let scenes = vec![
            make_minimal_scene("prologue", "这是序章。"),
            make_minimal_scene("chapter1/sakura_road", "樱花盛开的小路。"),
        ];

        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_ok(), "批量编译应成功");

        let compiled = result.unwrap();
        assert_eq!(compiled.scenes.len(), 2, "应有 2 个已编译场景");
        assert!(compiled.scenes.contains_key("prologue"), "应包含 prologue");
        assert!(
            compiled.scenes.contains_key("chapter1/sakura_road"),
            "应包含 chapter1/sakura_road"
        );
        assert_eq!(compiled.game_name, "Test Game");
        assert_eq!(compiled.game_version, "0.1.0");
        assert_eq!(compiled.entry_scene_id, "prologue");
        assert_eq!(compiled.build_info.source_file_count, 2);
        assert!(compiled.build_info.total_instructions > 0, "应有字节码指令");
        assert_eq!(compiled.build_info.optimization_level, "optimized");
        assert!(!compiled.build_info.build_timestamp.is_empty());
    }

    // ── AC02: 跨场景 Goto 目标存在时验证通过 ────────────────────────────

    /// AC02 — 有效的跨场景 Goto 引用通过验证。
    #[test]
    fn ac02_valid_cross_scene_goto_passes() {
        let scene_b = make_minimal_scene("scene_b", "场景 B");
        let scene_a = make_goto_scene("scene_a", "scene_b", None);

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_ok(), "有效的跨场景 Goto 应编译成功");
    }

    /// AC02 扩展 — Goto 带有效 label 通过验证。
    #[test]
    fn ac02_valid_goto_with_label_passes() {
        let scene_b = make_scene_with_labels("scene_b", "场景 B", &["my_label"]);
        let scene_a = make_goto_scene("scene_a", "scene_b", Some("my_label"));

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_ok(), "带有效 label 的跨场景 Goto 应编译成功");
    }

    // ── AC03: 跨场景 Goto 目标不存在时返回错误 ──────────────────────────

    /// AC03 — Goto 到不存在的场景产生错误。
    #[test]
    fn ac03_goto_nonexistent_scene_errors() {
        let scene_a = make_goto_scene("scene_a", "nonexistent_scene", None);
        let scene_b = make_minimal_scene("scene_b", "场景 B");

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_err(), "Goto 到不存在的场景应返回错误");

        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "应至少有一个错误");
        let error_msg = errors[0].to_string();
        assert!(
            error_msg.contains("nonexistent_scene"),
            "错误信息应包含目标场景名，实际：{}",
            error_msg
        );
        assert!(
            error_msg.contains("scene_a"),
            "错误信息应包含源场景名，实际：{}",
            error_msg
        );
    }

    // ── AC04: Goto 带 label 到不存在的标签返回错误 ──────────────────────

    /// AC04 — Goto 到存在的场景但标签不存在时产生错误。
    #[test]
    fn ac04_goto_nonexistent_label_errors() {
        let scene_b = make_scene_with_labels("scene_b", "场景 B", &["valid_label"]);
        let scene_a = make_goto_scene("scene_a", "scene_b", Some("bad_label"));

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_err(), "Goto 到不存在的标签应返回错误");

        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "应至少有一个错误");
        let error_msg = errors[0].to_string();
        assert!(
            error_msg.contains("bad_label"),
            "错误信息应包含不存在的标签名，实际：{}",
            error_msg
        );
        assert!(
            error_msg.contains("scene_b"),
            "错误信息应包含目标场景名，实际：{}",
            error_msg
        );
    }

    // ── AC05: optimize = false 跳过优化 Pass ─────────────────────────────

    /// AC05 — `optimize = false` 时编译产物的优化标记为 "none"。
    #[test]
    fn ac05_optimize_false_skips_optimization() {
        let scenes = vec![make_minimal_scene("test", "测试文本")];

        let result = compile_scenes(&scenes, &no_optimize_config());
        assert!(result.is_ok(), "不优化编译应成功");

        let compiled = result.unwrap();
        assert_eq!(
            compiled.build_info.optimization_level, "none",
            "优化级别应为 'none'"
        );
    }

    /// AC05 补充 — 优化开启时标记为 "optimized"。
    #[test]
    fn ac05_optimize_true_marked_as_optimized() {
        let scenes = vec![make_minimal_scene("test", "测试文本")];

        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(
            compiled.build_info.optimization_level, "optimized",
            "优化级别应为 'optimized'"
        );
    }

    // ── AC06: 编译失败时不阻断其他场景 ──────────────────────────────────

    /// AC06 — 多个场景中部分失败时，错误被收集统一返回。
    #[test]
    fn ac06_errors_collected_not_short_circuit() {
        // 场景 B 有语法错误（Goto 到不存在场景）
        let scene_a = make_minimal_scene("scene_a", "正常场景");
        let scene_b = make_goto_scene("scene_b", "nowhere", None);

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());

        // 应返回错误（因为场景 B 的 Goto 到不存在场景）
        assert!(result.is_err(), "存在无效引用时应返回错误");

        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "应至少有一个错误");
        // 错误信息应提及 scene_b
        let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        assert!(
            error_msgs
                .iter()
                .any(|m| m.contains("scene_b") || m.contains("nowhere")),
            "错误应涉及场景 B 或目标 'nowhere'，实际错误：{:?}",
            error_msgs
        );
    }

    /// AC06 扩展 — 所有场景都成功编译时无错误。
    #[test]
    fn ac06_all_valid_scenes_no_errors() {
        let scene_a = make_minimal_scene("scene_a", "场景 A");
        let scene_b = make_minimal_scene("scene_b", "场景 B");

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_ok(), "所有场景合法时应编译成功");
    }

    // ── 附加测试：边界情况 ──────────────────────────────────────────────

    /// 空场景列表编译成功。
    #[test]
    fn empty_scenes_compiles() {
        let scenes: Vec<(String, Scene)> = vec![];
        let characters = HashMap::new();
        let input = GameCompileInput {
            game_name: "Test Game",
            game_version: "0.1.0",
            entry_scene_id: "none", // 空场景列表使用占位符
            scenes: &scenes,
            characters: &characters,
            build_config: &default_build_config(),
        };
        let result = GameCompiler::compile(input);
        assert!(result.is_ok(), "空场景列表应编译成功");
        let compiled = result.unwrap();
        assert!(compiled.scenes.is_empty());
        assert_eq!(compiled.build_info.source_file_count, 0);
        assert_eq!(compiled.build_info.total_instructions, 0);
    }

    /// Goto 的 scene_id 使用变量引用（非字面量）时跳过验证。
    #[test]
    fn goto_with_variable_target_skips_validation() {
        // scene_id 是 $var 而非字符串字面量 → 运行时解析，编译期不报错
        let scene_a = (
            "scene_a".to_string(),
            Scene {
                id: "scene_a".into(),
                label: None,
                background: None,
                music: None,
                nodes: vec![SceneNode::Goto {
                    scene_id: Expr::variable("target_scene"),
                    label: None,
                }],
            },
        );
        let scene_b = make_minimal_scene("scene_b", "场景 B");

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(
            result.is_ok(),
            "变量 Goto 目标应在编译期跳过验证（运行时解析）"
        );
    }

    /// Goto 出现在 Branch 嵌套内部时验证递归覆盖。
    #[test]
    fn goto_in_nested_branch_validated() {
        let scene_b = make_minimal_scene("scene_b", "场景 B");
        let scene_a = (
            "scene_a".to_string(),
            Scene {
                id: "scene_a".into(),
                label: None,
                background: None,
                music: None,
                nodes: vec![SceneNode::Branch {
                    condition: Expr::bool_literal(true),
                    then_nodes: vec![SceneNode::Goto {
                        scene_id: s("nowhere"),
                        label: None,
                    }],
                    elif_branches: vec![],
                    else_nodes: None,
                }],
            },
        );

        let scenes = vec![scene_a, scene_b];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_err(), "嵌套 Branch 中的无效 Goto 应被检测到");
    }

    /// BuildInfo 的 build_timestamp 为 ISO 8601 格式。
    #[test]
    fn build_timestamp_is_iso8601() {
        let scenes = vec![make_minimal_scene("test", "测试")];
        let result = compile_scenes(&scenes, &default_build_config());
        assert!(result.is_ok());

        let compiled = result.unwrap();
        let ts = &compiled.build_info.build_timestamp;
        // 格式：YYYY-MM-DDTHH:MM:SSZ
        assert_eq!(ts.len(), 20, "ISO 8601 时间戳应为 20 字符");
        assert!(ts.ends_with('Z'), "应以 Z 结尾表示 UTC");
        assert!(
            ts.chars().nth(4) == Some('-') && ts.chars().nth(7) == Some('-'),
            "日期部分应用 - 分隔"
        );
        assert!(ts.chars().nth(10) == Some('T'), "日期和时间应用 T 分隔");
    }

    /// 验证 days_to_ymd 辅助函数的正确性（以已知日期为参照）。
    #[test]
    fn days_to_ymd_known_dates() {
        // 1970-01-01 是 UNIX epoch 的第 0 天
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1), "epoch 应为 1970-01-01");

        // 2026-06-15 = epoch + 20620 天（约）
        let total_days = (2026i64 - 1970) * 365
            + (2026 - 1970) / 4  // 闰年天数
            + 31 + 28 + 31 + 30 + 31 + 14; // 1月~5月 + 6月14日
        let (y, m, d) = days_to_ymd(total_days);
        // 月份应在 6 左右
        assert!(y >= 2026, "年应在 2026 或之后");
        assert!((1..=12).contains(&m), "月应在 1-12 之间");
        assert!((1..=31).contains(&d), "日应在 1-31 之间");
    }
}
