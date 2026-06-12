# Asterism（群星）— CLAUDE.md

> **项目 AI 行为宪章**
> 本文档约束所有 AI 开发会话（Claude）在本项目中的编码行为。
> 严格遵守本规范是纯 Vibe Coding 项目质量的基础保障。

---

## 一、项目概述

| 属性 | 值 |
|------|-----|
| 项目名称 | Asterism（群星） |
| 项目类型 | 开源 Galgame/ADV 游戏引擎 + IDE |
| 技术栈 | Rust（引擎）、Tauri v2 + Vue 3 + TypeScript（IDE）、.aster DSL（游戏脚本） |
| 许可协议 | MIT |
| 目标 | 为视觉小说创作者提供专业级制作工具 |

---

## 二、编码规范

### 2.1 Rust 编码规范（引擎）

#### 2.1.1 命名规范

| 元素 | 命名风格 | 示例 |
|------|---------|------|
| Crate 名称 | `kebab-case` | `aster-core`, `aster-renderer`, `aster-ui` |
| 模块/文件 | `snake_case` | `scene_manager.rs`, `asset_loader.rs` |
| Trait | `PascalCase` | `Renderer`, `AudioSystem`, `Platform` |
| Struct / Enum | `PascalCase` | `SceneManager`, `AssetId`, `SceneNode` |
| Enum Variant | `PascalCase` | `SceneNode::Dialogue`, `AssetType::Background` |
| 函数/方法 | `snake_case` | `load_scene()`, `play_bgm()`, `set_background()` |
| 常量/静态 | `SCREAMING_SNAKE_CASE` | `MAX_SAVE_SLOTS`, `DEFAULT_FPS` |
| 变量/参数 | `snake_case` | `asset_id`, `scene_path`, `fade_duration` |
| 私有字段 | `snake_case`（不加前缀） | `cache: LruCache<...>` |
| 泛型参数 | 单个大写字母（有意义） | `T` / `A: AssetLoader` / `S: AsRef<str>` |

#### 2.1.2 代码组织

```rust
// 文件头部注释（详见第三章）
// mod 声明顺序：
// 1. 标准库
// 2. 第三方 crate
// 3. 本 workspace 内 crate
// 4. 本地模块

use std::path::PathBuf;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{info, error, instrument};

use aster_core::{Scene, AssetId, VariableStore};
use aster_platform::Platform;

use crate::scene_manager::SceneState;
use crate::error::RuntimeError;
```

#### 2.1.3 类型使用规范

- **必须** 使用 Rust 2024 edition
- **必须** 为所有公开类型实现 `Debug`；数据类额外实现 `Clone + Serialize + Deserialize`
- **必须** 为 ID 类使用 newtype 模式：`pub struct AssetId(pub u64);`
- **禁止** 使用 `unwrap()` / `expect()` 在非测试代码中。使用 `?` 或 `match` 传播错误
- **禁止** 使用 `unsafe` 代码，除非 wgpu/FFI 接口需要且经过充分注释
- **优先** 使用 `&str` 而非 `String` 作为函数参数（除非函数需要持有所有权）
- **优先** 使用 `impl Trait` 而非泛型参数（当 trait 不复杂时）
- **必须** 对可能为空的返回使用 `Option<T>` 而非哨兵值（如 `-1`、`""`）

#### 2.1.4 架构约束

- 每个 engine crate 有且只有一个职责。如不确定拆分是否合理，优先拆分
- Crate 间依赖必须遵循分层架构：`aster-platform` ← `aster-core` ← 上层 crate
- 禁止循环依赖。`aster-core` 不依赖任何其他 engine crate
- 跨 crate 调用通过 trait 接口，而非直接依赖具体类型
- 热点路径（渲染循环、VM 执行）避免 trait object 虚调用，使用 enum dispatch 或泛型
- IDE 后端（Tauri）以库形式依赖 `aster-parser`、`aster-compiler`、`aster-core`，不依赖其他 engine crate
- 引擎不依赖任何 web/IDE 相关技术

#### 2.1.5 错误处理规范

```rust
// 库 crate（aster-* 除 runtime 外）：使用 thiserror 定义结构化错误
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("第{line}行第{col}列：{message}")]
    SyntaxError { line: usize, col: usize, message: String },

    #[error("语法错误：{0}")]
    Generic(String),

    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}

// 应用 crate（aster-runtime）：使用 anyhow 简化传播
pub fn run(config: &Config) -> anyhow::Result<()> {
    let platform = create_platform()?;
    let renderer = create_renderer(&platform).context("初始化渲染器失败")?;
    // ...
}

// 所有错误信息必须包含：
// 1. 出错的上下文（在干什么时出错）
// 2. 具体原因
// 3. 可能的修复建议（对创作者友好的错误）
```

#### 2.1.6 安全编码规范

- 所有外部输入（脚本文件内容、用户按键、资源文件）必须校验合法性
- 资源文件加载前检查魔数（magic bytes），防止加载非预期格式
- 存档文件必须校验 CRC32 完整性，加载前验证
- 禁止在代码中硬编码任何密钥、密码、Token
- wgpu shader 不接受来自游戏资源的 WGSL 代码（防止恶意 shader 攻击 GPU）——v1.0.0 的 custom_shaders 仅允许配置参数，不允许代码注入

### 2.2 Vue 3 + TypeScript 编码规范（IDE）

#### 2.2.1 命名规范

| 元素 | 命名风格 | 示例 |
|------|---------|------|
| Vue 组件文件 | `PascalCase` | `ScriptEditor.vue`, `AssetPanel.vue` |
| 组件目录 | `kebab-case` | `components/script-editor/` |
| Composables | `camelCase`，前缀 `use` | `useProjectStore()`, `useThemeLoader()` |
| TypeScript 接口 | `PascalCase`，不加 `I` 前缀 | `ProjectMeta`, `BuildResult` |
| TypeScript 类型别名 | `PascalCase` | `AssetType`, `DiagnosticLevel` |
| 变量/函数 | `camelCase` | `currentScene`, `loadProject()` |
| 常量 | `SCREAMING_SNAKE_CASE` | `MAX_RECENT_PROJECTS` |
| Pinia Store | `camelCase`，后缀 `Store` | `projectStore`, `editorStore` |
| Props 属性 | `camelCase` | `projectName`, `isModified` |

#### 2.2.2 组件结构

```vue
<!-- 组件文件结构规范 -->
<script setup lang="ts">
// 1. 导入声明
import { ref, computed, onMounted } from 'vue';
import { useProjectStore } from '@/stores/project';
import type { AssetInfo } from '@/types';

// 2. Props 和 Emits
const props = defineProps<{
  assetPath: string;
  readonly?: boolean;
}>();

const emit = defineEmits<{
  selected: [asset: AssetInfo];
  deleted: [path: string];
}>();

// 3. Composables
const projectStore = useProjectStore();

// 4. 响应式状态
const isLoading = ref(false);
const items = ref<AssetInfo[]>([]);

// 5. 计算属性
const filteredItems = computed(() =>
  items.value.filter(i => i.type === selectedType.value)
);

// 6. 方法
async function loadAssets(): Promise<void> {
  isLoading.value = true;
  // ...
}

// 7. 生命周期
onMounted(() => {
  loadAssets();
});
</script>

<template>
  <!-- 模板 -->
</template>

<style scoped>
/* 组件样式 */
</style>
```

#### 2.2.3 类型使用规范

- **必须** 使用 TypeScript strict mode（`tsconfig.json` 中 `strict: true`）
- **禁止** 使用 `any` 类型。遇到不确定的类型时使用 `unknown` 并通过类型守卫收窄
- **必须** 为所有 Tauri `invoke()` 调用定义返回类型
- **优先** 使用 `interface` 定义对象类型；`type` 用于联合/交叉/工具类型
- **必须** 为 Pinia Store 定义完整的 TypeScript 类型

#### 2.2.4 API 调用规范

```typescript
// 所有 Tauri IPC 调用通过统一的封装层
// 文件：src/api/index.ts

import { invoke } from '@tauri-apps/api/core';

// 封装 invoke，添加统一的错误处理和类型
export async function apiInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    // 统一错误处理：格式化并上报
    console.error(`[API] ${command} 失败:`, error);
    throw error;
  }
}

// 各功能模块的 API 调用封装
export async function checkSyntax(source: string): Promise<Diagnostic[]> {
  return apiInvoke<Diagnostic[]>('check_syntax', { source });
}
```

### 2.3 .aster DSL 设计规范

- 语法采用缩进敏感风格，缩进为 2 空格
- 关键字使用英文小写（`scene` / `show` / `menu` / `if` / `jump` 等）
- 字符串字面量使用双引号 `"`
- 注释使用 `--` 前缀
- 变量引用使用 `$` 前缀 (`$affection_score`)
- 新增语法特性必须先更新 `.aster` PEG 语法文件，再实现解析/编译
- 所有新语法必须在 `docs/aster-lang-reference.md` 中记录

---

## 三、Git 规范

### 3.1 分支策略

| 分支类型 | 命名格式 | 示例 | 说明 |
|---------|---------|------|------|
| 主分支 | `main` | — | 始终可构建，已发布的稳定版本 |
| 开发分支 | `develop` | — | 日常开发集成 |
| 功能分支 | `feat/<task-id>-<short-desc>` | `feat/T1-040-asset-manager` | 对应 Roadmap 任务编号 |
| 修复分支 | `fix/<short-desc>` | `fix/save-thumbnail-crash` | Bug 修复 |
| 重构分支 | `refactor/<short-desc>` | `refactor/extract-ui-crate` | 无功能变更的结构优化 |
| 发布分支 | `release/v<version>` | `release/v0.1.0` | 发布前稳定期 |
| 热修复 | `hotfix/<short-desc>` | `hotfix/v0.1.1-crash-on-start` | 紧急修复 |

### 3.2 Commit 规范（Conventional Commits）

```
<type>(<scope>): <subject>

[body]

[footer]
```

**Type 必须为以下之一**：

| Type | 说明 | 示例 |
|------|------|------|
| `feat` | 新功能 | `feat(renderer): 添加场景转场特效 fade 和 dissolve` |
| `fix` | Bug 修复 | `fix(vm): 修复条件跳转时变量未初始化导致的 panic` |
| `docs` | 文档变更 | `docs(architecture): 更新渲染管线架构图` |
| `style` | 代码格式（不影响功能） | `style(core): cargo fmt` |
| `refactor` | 代码重构（无功能变更） | `refactor(parser): 提取词法分析到独立模块` |
| `perf` | 性能优化 | `perf(renderer): 使用 sprite batching 减少 draw call` |
| `test` | 测试相关 | `test(vm): 添加字节码执行器集成测试` |
| `chore` | 构建/工具/依赖更新 | `chore(ci): 添加 macOS ARM64 构建目标` |

**Scope 为 crate/模块名称**：`core`, `parser`, `compiler`, `vm`, `renderer`, `audio`, `ui`, `asset`, `save`, `runtime`, `platform`, `ide`, `packager`

**Subject 规则**：
- 使用中文描述
- 不超过 72 字符
- 不加句号结尾
- 使用祈使语气（"添加"而非"添加了"）

### 3.3 提交前自检清单

在提交代码前，AI 必须自行完成以下检查：

- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过（零 warning）
- [ ] `cargo test --workspace` 通过
- [ ] `pnpm --dir ide typecheck` 通过
- [ ] `pnpm --dir ide lint` 通过
- [ ] 新增的公开函数/类型有完整的中文 docstring / JSDoc
- [ ] 没有遗留的 `unwrap()` / `expect()` / `todo!()` / `unimplemented!()`（除非有明确的 Issue 追踪）
- [ ] 没有硬编码的魔法数字（均应定义为命名常量）
- [ ] 新增的 crate 依赖已在 Architecture.md 中记录
- [ ] 新功能已更新 Requirements.md 中的对应需求状态

### 3.4 .gitignore 要求

以下文件和目录**必须**被 git 忽略：

```
# Rust
/target/
**/*.rs.bk

# Node.js
/node_modules/
/dist/

# IDE
.aster_cache/

# 系统文件
.DS_Store
Thumbs.db
*.swp
*.swo

# 环境配置
.env
.env.local
```

**禁止**提交以下内容：
- 编译产物（`target/`、`dist/`）
- 依赖目录（`node_modules/`）
- 操作系统的元数据文件
- 用户的本地 IDE 配置（`.vscode/` 除外，允许共享推荐扩展和调试配置）

---

## 四、注释规范

> **核心原则：注释必须详细、完整、规范、全面。这是纯 Vibe Coding 项目的基础保障。好的注释让 AI 在后续开发中能快速理解上下文，而不需要重新推理。**
>
> **注释语言：使用简体中文。**

### 4.1 文件级注释

每个源代码文件（Rust `.rs`、TypeScript `.ts`、Vue `.vue`）的**前 10 行内**必须包含文件头注释：

```rust
//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/sprite_batcher.rs
//! 功能概述：精灵批处理器 — 负责每帧收集所有 Sprite 绘制命令，按纹理合并为最小化 draw call
//!           支持最多 2048 个精灵/帧，超出时自动分批
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-20
//! 
//! 依赖模块：
//! - aster_core::AssetId（纹理资源标识）
//! - wgpu（GPU 渲染后端）
```

```typescript
/**
 * Asterism IDE — Vue 3 前端
 *
 * 文件路径：ide/src/components/editor/ScriptEditor.vue
 * 功能概述：脚本编辑器组件 — 封装 Monaco Editor，提供 .aster DSL 语法高亮、
 *           诊断信息展示（红色/黄色波浪线）、Ctrl+S 保存、与 Pinia editorStore 双向绑定
 * 作者：Claude (AI)
 * 创建日期：2026-06-18
 * 最后修改：2026-06-18
 */

// ... 组件代码
```

### 4.2 模块/包注释

```rust
//! ## aster-parser — .aster DSL 解析器
//!
//! 负责将 `.aster` 源码文件解析为抽象语法树（AST）。
//!
//! ### 解析流程
//! ```text
//! .aster 源码 → pest::Parser (PEG 语法) → PestToken 流 → AST Builder → ParsedScene
//! ```
//!
//! ### 对外接口
//! - `parse(source: &str) -> Result<ParsedScene, Vec<ParseError>>` — 解析完整场景
//! - `parse_expression(source: &str) -> Result<Expr, ParseError>` — 解析单个表达式
//! - `ParseError` — 携带行号/列号、错误消息、修复建议的结构化错误
//!
//! ### 使用示例
//! ```rust,ignore
//! use aster_parser::parse;
//! let scene = parse(include_str!("test_data/prologue.aster"))?;
//! ```
```

### 4.3 类/接口/类型注释

**Rust：**
```rust
/// 场景节点枚举 — 表示视觉小说中的一个基本演出单元。
///
/// 所有场景由一系列 SceneNode 组成，VM 按顺序执行每个节点。
/// 部分节点（Dialogue、Menu）会暂停等待用户输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SceneNode {
    /// 对话节点：显示说话者的对话文本
    /// - speaker: 说话者名称（对应 Character.id）
    /// - text: 对话内容，支持 inline markup 文本特效
    /// - voice_id: 可选的语音文件资源 ID
    Dialogue {
        speaker: String,
        text: String,
        voice_id: Option<AssetId>,
    },

    /// 菜单/选择支节点：显示一组选项并等待玩家选择
    /// - choices: 选项列表，每个选项包含文本和跳转目标
    /// - 当 choices 为空时，此为脚本错误
    Menu {
        choices: Vec<Choice>,
    },
    // ...
}
```

**TypeScript：**
```typescript
/**
 * 诊断信息 — 脚本语法检查的结果。
 *
 * 用于 Monaco Editor 的 markers 系统（setModelMarkers），
 * 在编辑器中以波浪线形式标注错误和警告。
 */
export interface Diagnostic {
  /** 严重级别：Error(红色波浪) | Warning(黄色波浪) | Info(蓝色波浪) */
  level: 'error' | 'warning' | 'info';

  /** 错误消息，中文 */
  message: string;

  /** 源文件中的行号（1-based，与 Monaco 一致） */
  line: number;

  /** 源文件中的列号（1-based） */
  column: number;

  /** 出错代码片段的字符数 */
  length: number;

  /** 修复建议（可选），显示在 tooltip 中 */
  hint?: string;
}
```

### 4.4 函数/方法注释

**Rust — 公共函数必须包含完整 JSDoc 风格的 doc comment：**
```rust
/// 加载并解析场景脚本文件。
///
/// 执行三步流程：
/// 1. 从文件系统读取 `.aster` 或 `.asterbyte` 文件
/// 2. 如果是 `.aster` 源码，则解析→编译；如果是 `.asterbyte`，则直接反序列化
/// 3. 将编译后的场景加载到 VM，准备执行
///
/// # 参数
/// - `scene_id`: 场景标识符（如 "chapter1/prologue"），对应 `scripts/` 下的文件路径
/// - `asset_manager`: 资源管理器，用于读取脚本文件和关联的音频/图片资源
///
/// # 返回值
/// - `Ok(CompiledScene)`: 编译完成的场景，包含字节码和元数据
/// - `Err(LoadSceneError)`: 加载失败的原因
///   - `NotFound`: 场景文件不存在
///   - `ParseError`: 脚本语法错误（含位置信息）
///   - `CompileError`: 脚本语义错误
///   - `IoError`: 文件系统错误
///
/// # 性能
/// - 10k 行 `.aster` 脚本解析+编译耗时 < 300ms
/// - 预编译 `.asterbyte` 加载耗时 < 10ms
///
/// # 示例
/// ```rust,ignore
/// let scene = scene_manager.load_scene("prologue", &asset_manager)?;
/// vm.execute(&scene)?;
/// ```
pub fn load_scene(
    &mut self,
    scene_id: &str,
    asset_manager: &AssetManager,
) -> Result<CompiledScene, LoadSceneError> {
    // ...
}

/// 私有/内部函数至少包含功能描述和参数说明。
/// 当前光标位置是否在任意 UI 控件的热区内。
///
/// # 参数
/// - `cursor_pos`: 当前鼠标在窗口坐标系中的位置（原点为左上角）
///
/// # 返回值
/// - `Some(WidgetId)`: 鼠标所在控件的 ID
/// - `None`: 鼠标未在任何可交互控件上方
fn hit_test(&self, cursor_pos: (f32, f32)) -> Option<WidgetId> {
    // ...
}
```

**TypeScript：**
```typescript
/**
 * 将游戏项目构建为可独立运行的安装包。
 *
 * 构建流程：
 * 1. 编译所有 .aster 脚本 → .asterbyte
 * 2. 复制并优化资源文件
 * 3. 生成平台安装包（NSIS/DMG/AppImage）
 *
 * @param projectPath - 项目根目录的绝对路径
 * @param platform - 目标平台：'windows' | 'macos' | 'linux'
 * @param options - 构建选项
 * @param options.release - 是否为正式发布构建（true=优化+压缩）
 * @param options.encryptAssets - 是否加密资源归档（默认 false）
 * @returns 构建结果，包含输出路径和每个步骤的日志
 * @throws {BuildError} 编译失败、资源缺失或平台工具链不可用
 *
 * @example
 * ```ts
 * const result = await buildProject('/path/to/project', 'windows', {
 *   release: true,
 *   encryptAssets: false,
 * });
 * console.log(`构建完成: ${result.outputPath}`);
 * ```
 */
export async function buildProject(
  projectPath: string,
  platform: 'windows' | 'macos' | 'linux',
  options: BuildOptions,
): Promise<BuildResult> {
  // ...
}
```

### 4.5 内联注释

```rust
// 内联注释规范：

// 1. 复杂算法的每一步必须有解释
// 步骤1：检测所有已显示角色中哪些正在播放语音
// 步骤2：对每个角色，分析语音波形的振幅包络
// 步骤3：将振幅映射为 viseme（口型）序列（A/I/U/E/O 五个基本口型）

// 2. 非直观的决策必须注释原因
// 使用固定 16 个寄存器而非动态分配 —— 通过 profile 发现，动态寄存器
// 会导致 VM dispatch 循环中的分支预测失败率增加 40%

// 3. 魔法数字和硬编码值必须注释来源
const TEXTURE_CACHE_SIZE: usize = 256 * 1024 * 1024; // 256 MB
// 来源：实测 1080p 项目单个场景平均使用 180MB 纹理，256MB 提供约 40% 余量

// 4. 临时方案 / TODO / HACK 必须标记
// TODO(Claude): v0.4 引入 ZSTD 压缩后，缓冲区大小需根据压缩率动态计算
// 当前固定分配 2MB，对 4K 素材可能不够

// HACK: wgpu 23.0.1 在 macOS Metal 后端存在 buffer 对齐 bug，
// 当前通过手动填充 256 字节对齐绕过。跟踪 issue: wgpu#4567
// 一旦修复，移除本 hack
```

### 4.6 类型注解

- **Rust**：所有 `pub fn` 的参数和返回值必须有显式类型注解（不允许省略类型的闭包参数除外）
- **TypeScript**：所有函数参数和返回值必须有显式类型注解
- **禁止**：`as any` 类型断言、`@ts-ignore`、`@ts-expect-error`（除非有注释说明原因）

### 4.7 API 端点注释

Tauri Commands（Rust 后端 → 前端 IPC）必须注释：

```rust
/// Tauri Command: check_syntax
///
/// 检查 .aster 脚本源码的语法正确性，返回诊断信息列表。
///
/// ## 路径
/// `invoke('check_syntax', { source: string })`
///
/// ## 参数（来自前端 invoke）
/// - `source`: 完整的 .aster 脚本源码字符串
///
/// ## 返回
/// `Promise<Diagnostic[]>` — 诊断信息列表，空列表表示无错误
///
/// ## 预设错误码
/// - 无错误：返回 `[]`
/// - 语法错误：返回包含 line/column/message/hint 的 Diagnostic 数组
/// - 内部错误：返回单个 error 级别 Diagnostic，message 包含内部错误描述
///
/// ## 性能
/// 10k 行源码解析 < 100ms，超时则后台执行并返回空结果（异步后续更新）
#[tauri::command]
async fn check_syntax(source: String) -> Result<Vec<Diagnostic>, String> {
    // ...
}
```

---

## 五、测试规范

### 5.1 测试要求

| 层级 | 覆盖率要求 | 框架 |
|------|-----------|------|
| 引擎 crate 单元测试 | ≥ 80% | `cargo test` + `rstest` |
| 引擎集成测试 | 覆盖每个 P0 需求 | `cargo test`（`tests/` 目录） |
| 渲染器快照测试 | 关键场景 | 自定义 snapshot test harness |
| IDE 前端单元测试 | 关键组件 | Vitest + Vue Test Utils |
| IDE 后端测试 | Tauri Commands | `cargo test` |
| E2E 测试 | 核心用户流程 | Playwright |

### 5.2 测试命名

```rust
// Rust
#[test]
fn test_scene_manager_loads_valid_scene() { }
#[test]
fn test_scene_manager_returns_error_on_missing_file() { }
#[test]
fn test_vm_executes_conditional_jump_correctly() { }
```

---

## 六、会话行为准则

每次 AI 开发会话必须遵循以下流程：

1. **会话开始**：阅读本 CLAUDE.md，了解项目约定
2. **任务选择**：参考 Roadmap.md 中当前 Phase 的任务列表
3. **编码实现**：严格遵循本文档定义的编码规范、注释规范
4. **自检**：完成 3.3 节的自检清单
5. **文档同步**：如有架构变更，同步更新 Architecture.md
6. **任务状态更新**：在 Roadmap.md 中更新对应任务状态
7. **会话结束**：确认 `cargo test --workspace` 和 `pnpm --dir ide typecheck` 通过

---

*本文档与项目共同演进。任何规范变更应通过 PR 提议，并在 `.claude/CLAUDE.md` 中更新。*
