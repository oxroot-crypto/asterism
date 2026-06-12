# Asterism（群星）— 项目路线图

> **文档版本**：v1.1
> **创建日期**：2026-06-12
> **最后修改**：2026-06-12
> **作者**：Claude (AI)
> **上游文档**：[Solution.md](./Solution.md) → [Requirements.md](./Requirements.md) → [Architecture.md](./Architecture.md)

---

## 1. 里程碑概览

| 里程碑 | 名称 | 预计工期 | 核心交付 | 引擎 Crate | IDE 功能 |
|--------|------|---------|---------|-----------|---------|
| **v0.0.1-dev** | Phase 0 — 项目初始化 | 2 周 | 脚手架 + CI/CD + 规范 | workspace 骨架 | Tauri 骨架 |
| **v0.1.0-alpha** | Phase 1 — 引擎基础：脚本解析与渲染管线 | 8 周 | .aster 脚本 → GPU 像素的全管线 | platform, core, parser, compiler, vm, renderer, runtime(basic) | — |
| **v0.2.0-alpha** | Phase 2 — 引擎完整：音频、资源与存档 | 6 周 | 完整 VN 播放器（含音频/存档） | audio, asset, save | — |
| **v0.3.0-beta** | Phase 3 — IDE MVP | 6 周 | 脚本编辑、资源管理、一键构建 | — | 项目管理, Monaco, 构建管线 |
| **v0.4.0-beta** | Phase 4 — 游戏体验增强 | 8 周 | 转场 · 语音 · Skip · 画廊 · 增强DSL | renderer, audio, runtime, parser, compiler | — |
| **v0.5.0-rc** | Phase 5 — UI 主题系统与 IDE 增强 | 8 周 | 完整主题系统 + 丰富 IDE 功能 | ui | 预览, 流程图, 主题编辑器 |
| **v1.0.0-stable** | Phase 6 — 高级渲染与生态分发 | 14 周 | Live2D · 粒子 · Steam · 插件 · 文档 | plugin, steam(feature) | 时间线, 调试器, 发布向导 |

> **时间线说明**：以上为乐观估计，每 Phase 已包含约 30% 缓冲。作为纯 Vibe Coding 项目（AI 独立开发），实际进度可能因 AI 模型能力边界、技术探索成本等因素波动。每个 Phase 结束后根据实际 velocity 重新校准后续 Phase 的预估。

```
Time:  0w ── 2w ─────── 10w ────── 16w ────── 22w ────────── 30w ────────── 38w ──────────────────── 52w
Phase:  [P0] [    P1    ] [   P2   ] [   P3  ] [     P4     ] [     P5     ] [          P6            ]
Ver:   dev   v0.1.0-a    v0.2.0-a   v0.3.0-b  v0.4.0-b      v0.5.0-rc     v1.0.0-stable
Crates:      └─ 5 crates ─┘├─ +3 ──┤                                  └─ 11 crates total ──────────┘
```

> **Phase 间的依赖**：P1（引擎基础）是后续所有 Phase 的先决条件。P2（引擎完整）和 P3（IDE MVP）可部分并行——P3 的 Tauri 后端在 P1 的 parser/compiler 稳定后即可启动，无需等 P2 全部完成。P4/P5/P6 为严格串行，各自由前一个 Phase 的完整产物支撑。

---

## 2. Phase 0 — 项目初始化（Week 1-2）

### 2.1 目标

建立完整的开发基础设施，确保后续所有 Phase 的编码工作能高效、一致、自动化地进行。

### 2.2 任务清单

| 编号 | 任务 | 对应架构模块 | 预估工时 | 交付物 | 验收标准 |
|------|------|-------------|---------|--------|---------|
| T0-001 | 初始化 Cargo workspace | `engine/` 全部 crate | 2h | `Cargo.toml` workspace 文件，`engine/aster-*/` 目录骨架，所有 crate 的初始 `Cargo.toml` | `cargo build --workspace` 通过 |
| T0-002 | 初始化 Tauri + Vue 3 项目 | `ide/` | 4h | Tauri v2 + Vue 3 + TypeScript + Vite 项目骨架 | `pnpm tauri dev` 显示空白窗口 |
| T0-003 | 初始化 packager crate | `packager/` | 2h | `packager/` 目录骨架，初始 CLI 框架（clap） | `cargo run -- --help` 显示帮助 |
| T0-004 | CI/CD — Rust | `.github/workflows/` | 4h | `ci-rust.yml`：`cargo build` / `cargo test` / `cargo clippy` / `cargo fmt --check` 3 平台矩阵 | CI 绿标，任何 PR 必过 |
| T0-005 | CI/CD — IDE | `.github/workflows/` | 2h | `ci-ide.yml`：`pnpm lint` / `pnpm typecheck` / `pnpm test` / `pnpm build` + `pnpm tauri build`（3 平台矩阵） | CI 绿标 |
| T0-006 | CI/CD — Artifacts | `.github/workflows/` | 2h | 构建产物自动上传（引擎二进制 + IDE 二进制 + 前端 dist） | Release 页有可下载产物 |
| T0-007 | 编写 CLAUDE.md | `.claude/` | 4h | 编码规范 + Git 规范 + 注释规范 | 符合项目立项要求 |
| T0-008 | 编写四份核心文档 | `docs/` | 16h | Solution.md / Requirements.md / Architecture.md / Roadmap.md | 四份文档追踪链完整，覆盖所有要求章节 |
| T0-009 | 创建示例项目模板 | `templates/default_project/` | 2h | 默认项目骨架模板（IDE "新建项目" 复制此模板） | 包含标准目录结构和示例 .aster 脚本 |
| T0-010 | 搭建 issue 模板 | `.github/ISSUE_TEMPLATE/` | 1h | Bug Report / Feature Request / Question 模板 | 模板结构清晰 |
| T0-011 | 搭建 PR 模板 | `.github/pull_request_template.md` | 0.5h | PR 检查清单：clippy / fmt / test / 文档 | 清单完整 |
| T0-012 | 配置编译缓存加速 | `.cargo/config.toml` | 2h | `Swatinem/rust-cache@v2`（CI 缓存）+ sccache 可选（本地），增量编译 | `cargo build` 增量编译时间减少 ≥ 50% |
| T0-013 | 配置 debug 编译优化 | `engine/Cargo.toml` (workspace) | 0.5h | workspace 级别 `[profile.dev]` 设置 `opt-level = 1` | debug build 编译时间减少 ~30% |
| T0-014 | 创建 README.md | 项目根目录 | 2h | 项目介绍、快速开始、文档索引、许可信息、徽章 | README 包含所有必要入口信息 |

**Phase 0 产出物检查**：
- [ ] `cargo build --workspace` 通过（所有 crate 编译，CI 缓存命中率 > 50%）
- [ ] `cargo test --workspace` 通过（哪怕只有占位测试）
- [ ] `cargo clippy --workspace` 通过（无 warning）
- [ ] `cargo fmt --check` 通过
- [ ] `pnpm tauri dev` 能启动空白 IDE 窗口
- [ ] CI 全部绿灯（3 平台矩阵）
- [ ] `docs/` 目录下四份文档完整
- [ ] `.claude/CLAUDE.md` 存在且内容完整
- [ ] `README.md` 存在且包含项目介绍、快速开始、文档索引

---

## 3. Phase 1 — 引擎基础：脚本解析与渲染管线（Week 3-10，8 周）

### 3.1 目标

建立从 `.aster` 脚本源码到 GPU 像素的完整管线。Phase 1 结束时，引擎可以加载一个 `.aster` 脚本文件，在窗口中展示背景、角色立绘、对话文本（含打字机效果），并通过鼠标点击推进剧情。**不含音频和存档**——这些属于 Phase 2。

| 维度 | 范围 |
|------|------|
| **涉及的 Crate** | `aster-platform`, `aster-core`, `aster-parser`, `aster-compiler`, `aster-vm`, `aster-renderer`(basic), `aster-runtime`(basic) |
| **覆盖的 P0 需求** | REQ-ENG-001~003, REQ-ENG-010~014, REQ-ENG-020~023 |
| **明确不包含** | 音频、存档/读档、资源缓存、IDE 任何功能 |

### 3.2 任务 — 基础设施与类型系统（Week 3-4）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-001 | 实现 `aster-platform` | `aster-platform` | NFR-COMPAT-001~003 | 8h | `Platform` trait + 3 平台实现 + 单元测试 | 三个平台均能正确返回用户目录和存档路径 |
| T1-002 | 实现 `aster-core` 所有类型 | `aster-core` | REQ-ENG-003 | 12h | Project / Character / Scene / SceneNode / Asset / SaveData / Theme 等所有核心类型 + serde 派生 | 所有类型可序列化/反序列化，单元测试覆盖 |
| T1-003 | 实现 `aster-parser` — PEG 语法 + AST | `aster-parser` | REQ-ENG-001 | 16h | .aster PEG 语法文件 + AST 构建器 + 错误收集器 + 单元测试 | 有效脚本→AST、无效脚本→含行号的错误信息 |

### 3.3 任务 — 渲染管线（Week 4-7）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-010 | wgpu 设备初始化 + 窗口创建 | `aster-renderer` | REQ-ENG-010 | 8h | GpuContext 初始化与销毁，窗口 surface 配置 | 三个平台均显示窗口，可清屏为指定颜色 |
| T1-011 | 背景图层渲染 | `aster-renderer` | REQ-ENG-011 | 8h | 全屏四边形 Texture 加载与显示，宽高比适配（裁剪/留黑边） | 不同尺寸背景图正确显示，不拉伸变形 |
| T1-012 | 角色立绘渲染 | `aster-renderer` | REQ-ENG-012 | 12h | 带 Alpha 的 Sprite 渲染，位置/透明度可调，多立绘层级正确 | 3 个立绘同时显示，透明度渐变生效 |
| T1-013 | 文本渲染 | `aster-renderer` | REQ-ENG-013 | 12h | cosmic-text 集成，CJK+Latin 正确显示，基本色/字号/行距 | 中日文无乱码，对齐正确 |
| T1-014 | 打字机效果 | `aster-renderer` | REQ-ENG-014 | 8h | 字符逐字显示，速度可配，点击跳过全部文本 | 多档速度下逐字显示平滑，点击即完成 |

### 3.4 任务 — 脚本编译与执行（Week 6-8）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-020 | 实现 `aster-compiler` | `aster-compiler` | REQ-ENG-002 | 16h | AST→IR→Bytecode 编译管线 + 4 个优化 Pass + 单元测试 | 编译后字节码可被 VM 执行，优化后指令数 ≤ 优化前 |
| T1-021 | 实现 `aster-vm` 核心 | `aster-vm` | REQ-ENG-002, 003 | 16h | 寄存器 VM，token-threaded dispatch，全部操作码，VmAction 回调 | 所有字节码指令正确执行 |
| T1-022 | VM 变量/旗标/跳转 | `aster-vm` | REQ-ENG-003, 023 | 8h | VariableStore / FlagSet 操作，条件/无条件跳转，子例程调用 | `if/elif/else` 分支正确，`jump` 到目标标签 |

### 3.5 任务 — 运行时集成（Week 8-10）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-050 | 实现 SceneManager | `aster-runtime` | REQ-ENG-020~023 | 12h | 场景状态机，VM Action→Renderer 命令转换（音频/存档命令预留桩函数，记录 warn 日志而非崩溃） | 完整场景从加载到结束可运行 |
| T1-051 | 实现 DialogueController | `aster-runtime` | REQ-ENG-020~021 | 6h | 对话流管理，打字机状态控制，文本缓冲队列 | 对话推进正确，打字机等待→点击→完成流程无 bug |
| T1-052 | 实现 InputManager | `aster-runtime` | REQ-ENG-020~021 | 4h | winit 事件→游戏动作映射（Enter/Space/Click 推进，Esc 预留） | 鼠标和键盘推进行为一致，无重复触发 |
| T1-053 | 主事件循环 | `aster-runtime` | — | 8h | 帧循环（60fps），update→render→present 管线，窗口 resize/最小化处理 | 稳定 60fps，窗口 resize 正确重分配 swapchain |

**Phase 1 产出物检查**：
- [ ] `aster-platform`、`aster-core`、`aster-parser`、`aster-compiler`、`aster-vm` 单元测试通过
- [ ] 加载一个包含 bg + 1 角色 + 5 句对话 + 1 个选择支的 .aster 脚本，可完整播放
- [ ] 1080p 下稳定 60fps（基础渲染，无后处理）
- [ ] CJK 文本渲染无乱码
- [ ] 打字机效果流畅，点击跳过正常
- [ ] 三个桌面平台均通过冒烟测试
- [ ] 所有 P0 引擎需求（REQ-ENG-001~003, 010~014, 020~023）通过验收

---

## 4. Phase 2 — 引擎完整：音频、资源与存档（Week 11-16，6 周）

### 4.1 目标

将 Phase 1 的"静默播放器"升级为完整的视觉小说引擎。创作者可以用它播放带 BGM/SE 的场景，将游戏进度存档到磁盘并在之后恢复。引擎具备 LRU 资源缓存和基本的存档界面（引擎内 UI）。

| 维度 | 范围 |
|------|------|
| **新增 Crate** | `aster-audio`, `aster-asset`, `aster-save` |
| **覆盖的 P0 需求** | REQ-ENG-030~032, REQ-ENG-040~042 |
| **明确不包含** | Voice 通道、crossfade、多存档槽位 UI、IDE 任何功能 |

### 4.2 任务 — 音频系统（Week 11-12）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-030 | 实现 `aster-audio` — BGM/SE | `aster-audio` | REQ-ENG-030~032 | 12h | kira 集成，BGM 循环/停止，SE 播放，fade_in/fade_out | BGM 无缝循环，SE 不干扰 BGM，淡入淡出平滑无爆音 |
| T1-031 | 音频状态快照 | `aster-audio` | REQ-ENG-040 | 4h | 音频系统当前状态可序列化（用于存档恢复） | 读档后 BGM 从存档位置继续播放 |

### 4.3 任务 — 资源与存档（Week 12-14）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-040 | 实现 `aster-asset` | `aster-asset` | REQ-ENG-011, REQ-IDE-020 | 12h | 文件扫描/加载/缓存，LRU 淘汰，PNG/WebP→Texture，OGG/FLAC→AudioBuffer | 资源按需加载，缓存命中率可测量 |
| T1-041 | 实现 `aster-save` | `aster-save` | REQ-ENG-040~042 | 12h | SaveData 序列化/反序列化（version 字段 + CRC32），缩略图捕获，5 手动 + 1 快速 + 1 自动槽位 | 存档/读档正确恢复全量游戏状态（场景/变量/立绘/BGM） |

### 4.4 任务 — 集成测试（Week 14-16）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-054 | 引擎集成测试 — 基础流程 | `aster-runtime` | 全部 P0 引擎需求 | 12h | 端到端测试：加载项目→播放场景→存档→读档→退出。3 个平台各运行一次 | 完整流程无崩溃，3 平台通过 |
| T1-055 | 引擎集成测试 — 异常路径 | `aster-runtime` | 全部 P0 引擎需求 + NFR-SEC | 8h | 异常测试：资源缺失恢复、脚本语法错误提示、存档损坏检测、窗口最小化/恢复、分辨率切换 | 所有异常路径有优雅降级而非 panic |
| T1-056 | 引擎集成测试 — 性能验证 | `aster-runtime` | NFR-PERF-001~008 | 4h | 性能基准测试：1080p/4K 帧率、场景加载时间、内存占用 | 所有指标满足 NFR-PERF 目标值 |

**Phase 2 产出物检查**：
- [ ] `aster-audio`、`aster-asset`、`aster-save` 单元测试通过
- [ ] 引擎可播放带 BGM + SE 的完整场景，音频与画面同步正确
- [ ] 存档/读档恢复全量状态（场景位置、变量、旗标、立绘、BGM 进度）
- [ ] 存档 CRC32 校验：损坏文件拒绝加载并给出提示
- [ ] 集成测试全绿（3 平台 × 3 种流程）
- [ ] 性能指标满足 NFR-PERF-001~008
- [ ] 可脱离 IDE 独立运行（`aster-runtime --project /path/to/project`）

---

## 5. Phase 3 — IDE MVP（Week 17-22，6 周）

### 5.1 目标

交付完整的集成开发环境。创作者无需接触命令行，在 IDE 中即可完成：创建项目 → 编写 .aster 脚本（含实时语法检查）→ 拖拽导入素材 → 一键构建安装包 → 启动预览。

| 维度 | 范围 |
|------|------|
| **涉及组件** | Tauri Rust 后端 + Vue 3 前端 |
| **依赖的引擎 Crate** | `aster-parser`(lib), `aster-compiler`(lib), `aster-core`(lib) |
| **覆盖的 P0 需求** | REQ-IDE-001~003, REQ-IDE-010~011, REQ-IDE-020~023 |

### 5.2 任务 — Tauri 后端（Week 17-19）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-060 | 项目管理 | `ide/src-tauri/` | REQ-IDE-001~003 | 8h | create_project / open_project / 文件树 API / 最近项目列表 | 创建/打开项目成功，文件树实时反映文件系统 |
| T1-061 | 构建管线 | `ide/src-tauri/` | REQ-IDE-022 | 8h | build_project 命令：编译.aster→复制资源→嵌入引擎→生成安装包 | 构建产物在目标目录且可运行 |
| T1-062 | 引擎桥接 | `ide/src-tauri/` | REQ-IDE-011 | 6h | 对接 aster-parser 和 aster-compiler，提供 check_syntax / compile_script 命令 | 语法错误正确返回 Diagnostic（line/col/message/hint） |
| T1-063 | 预览管理 | `ide/src-tauri/` | REQ-IDE-023 | 6h | launch_preview：启动引擎子进程 + IPC 通道 + 进程生命周期管理 | 预览窗口正常启动/关闭，IDE 退出时无僵尸进程 |

### 5.3 任务 — Vue 前端（Week 19-22）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 | 验收标准 |
|------|------|---------|---------|------|--------|---------|
| T1-064 | App Shell | `ide/src/` | — | 8h | Tauri 窗口框架，菜单栏，面板布局（可拖拽分割），快捷键绑定 | 多面板布局稳定，窗口 resize 面板自适应 |
| T1-065 | 文件树 | `ide/src/` | REQ-IDE-003 | 6h | TreeView 组件，右键菜单（新建/删除/重命名），文件图标，外部变更侦听 | 文件操作正确，与文件系统实时同步 |
| T1-066 | Monaco 编辑器 | `ide/src/` | REQ-IDE-010 | 8h | Monaco 集成，.aster 语法高亮（TextMate grammar），括号匹配，自动缩进，Ctrl+S | 语法高亮正确，编辑流畅无输入延迟 |
| T1-067 | 诊断显示 | `ide/src/` | REQ-IDE-011 | 4h | 错误行红色波浪下划线，悬停 tooltip 显示详情+修复建议，Problems 面板汇总 | 错误即时显示（500ms 内），点击跳转到精确行号 |
| T1-068 | 资源面板 | `ide/src/` | REQ-IDE-020~021 | 8h | 拖拽导入，自动归类到 sprites/bgm/se 等子目录，图片缩略图网格 | 拖拽导入成功，缩略图正确，大文件不阻塞 UI |
| T1-069 | 构建/预览按钮 | `ide/src/` | REQ-IDE-022~023 | 4h | 工具栏按钮 + 构建进度条 + 日志输出面板（实时追加构建步骤） | 构建过程日志实时显示 |
| T1-070 | .aster 语言参考（初稿） | `docs/aster-lang-reference.md` | REQ-ENG-001, REQ-DSL-* | 8h | .aster DSL 完整语法规范初稿：场景结构、对话/旁白/选择支、变量/旗标、跳转。含 BNF 式语法定义和代码示例 | 语法规范完整到可供 IDE 语法高亮和诊断功能参考 |

**Phase 3 产出物检查**：
- [ ] IDE 可创建/打开项目，文件树操作正常
- [ ] Monaco 编辑器语法高亮正确，Ctrl+S 保存
- [ ] 实时语法诊断：错误在 500ms 内以红色波浪线标注
- [ ] 拖拽 PNG/OGG 文件到资源面板，自动归类
- [ ] 一键构建生成可运行的安装包（Windows NSIS / macOS DMG / Linux AppImage 至少其一）
- [ ] 预览按钮启动引擎子进程，游戏窗口正常运行
- [ ] `docs/aster-lang-reference.md` 初稿完成
- [ ] 附带一个可运行的示例项目（≥1 场景 + 1 角色 + 1 选择支 + BGM）

---

## 6. Phase 4 — 游戏体验增强（Week 23-30，8 周）

### 6.1 目标

将引擎从"能用"提升到"专业"。引入场景转场特效、增强音频（Voice/Crossfade）、完整游戏系统（Skip/Auto/Backlog/CG 画廊/音乐鉴赏/设置），以及 DSL 宏/模板/表达式增强。

| 维度 | 范围 |
|------|------|
| **涉及的 Crate** | `aster-renderer`, `aster-audio`, `aster-runtime`, `aster-parser`, `aster-compiler` |
| **覆盖的 P1 需求** | REQ-ENG-050~052, REQ-ENG-060~062, REQ-ENG-070~078, REQ-DSL-001~003 |
| **明确不包含** | UI 主题系统（Phase 5）、IDE 增强功能（Phase 5） |

### 6.2 任务 — 渲染增强（Week 23-25）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T2-001 | 场景转场特效 | `aster-renderer` | REQ-ENG-050 | 16h | crossfade / fade_to_black / slide(上下左右) / dissolve / wipe，时长可配，WGSL shader |
| T2-002 | 角色图层动画 | `aster-renderer` | REQ-ENG-051 | 12h | show/hide 时 fade/slide 过渡动画，z-index 层次管理 |
| T2-003 | 文字特效系统 | `aster-renderer` | REQ-ENG-052 | 12h | shake / rainbow / ruby 注音 / bold+italic，通过 inline markup 声明，可叠加 |
| T2-004 | 背景模糊 pass | `aster-renderer` | — | 8h | 菜单/设置界面背景模糊效果（Gaussian blur shader），对帧率影响 < 5% |

### 6.3 任务 — 音频增强（Week 24-25）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T2-010 | 语音通道（Voice） | `aster-audio` | REQ-ENG-060 | 8h | Voice 独立通道，对话推进自动停止语音，Skip 模式静音 |
| T2-011 | 多轨独立控制 | `aster-audio` | REQ-ENG-061 | 4h | BGM/SE/Voice 独立音量/静音，设置界面集成 |
| T2-012 | Crossfade | `aster-audio` | REQ-ENG-062 | 6h | BGM 切换时交叉淡入淡出，无静音间隙，时长可配 |

### 6.4 任务 — 游戏系统（Week 25-28）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T2-030 | 多存档槽位 + 缩略图 | `aster-save` | REQ-ENG-070 | 12h | 30 槽位，缩略图截取（320×180），存档元信息展示，覆盖确认 |
| T2-031 | 快速/自动存档 | `aster-save` + `aster-runtime` | REQ-ENG-071~072 | 6h | F5/F9 快捷键（响应 < 200ms），场景切换自动存档，独立槽位 |
| T2-032 | Skip / Auto 模式 | `aster-runtime` | REQ-ENG-073~074 | 8h | Skip（跳过已读、选择支暂停），Auto（可配间隔 1-10s、选择支暂停） |
| T2-033 | 历史回顾（Backlog） | `aster-runtime` | REQ-ENG-075 | 8h | 最近 1000 条对话，滚动浏览，语音行可点击回放 |
| T2-034 | CG 画廊 + 音乐鉴赏 | `aster-runtime` + `aster-ui`(basic) | REQ-ENG-076~077 | 16h | CG 解锁/展示/全屏查看，音乐播放/进度/循环模式 |
| T2-035 | 设置面板 | `aster-runtime` + `aster-ui`(basic) | REQ-ENG-078 | 12h | 文字速度/自动间隔/各通道音量/全屏切换/按键绑定查看，设置持久化到磁盘 |

### 6.5 任务 — DSL 增强（Week 27-28）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T2-040 | 宏定义系统 | `aster-parser` + `aster-compiler` | REQ-DSL-001 | 12h | `macro` 定义/调用，参数替换，错误追踪到调用位置 |
| T2-041 | 场景模板/继承 | `aster-parser` + `aster-compiler` | REQ-DSL-002 | 8h | `extend` 语法，字段覆盖语义，多层继承 |
| T2-042 | 表达式增强 | `aster-parser` + `aster-compiler` | REQ-DSL-003 | 8h | 算术/字符串拼接/三元运算符/数组/映射字面量，类型不兼容报错 |

**Phase 4 产出物检查**：
- [ ] 所有 P1 需求通过验收
- [ ] 5 种转场特效均可正常使用，时长可配
- [ ] Voice/BGM/SE 三通道独立控制，crossfade 无间隙
- [ ] 30 槽存档 + 缩略图 + 快速/自动存档正常工作
- [ ] Skip/Auto/Backlog 交互正确
- [ ] CG 画廊和音乐鉴赏可用
- [ ] 设置面板所有选项即时生效且持久化保存
- [ ] 宏/模板/表达式增强语法正确编译执行

---

## 7. Phase 5 — UI 主题系统与 IDE 增强（Week 31-38，8 周）

### 7.1 目标

集成完整的 UI 主题系统（`aster-ui` crate）+ 丰富 IDE 功能。引擎侧：默认精美主题覆盖全部 12 个游戏内界面，支持 theme.toml 声明式定制。IDE 侧：实时预览面板、角色管理器、分支流程图、主题可视化编辑器。

| 维度 | 范围 |
|------|------|
| **新增 Crate** | `aster-ui` |
| **覆盖的 P1 需求** | REQ-UI-001~005, REQ-IDE-030~036 |

### 7.2 任务 — UI 主题系统（Week 31-35）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T2-020 | 默认精美主题实现 | `aster-ui` | REQ-UI-001 | 24h | 全部 12 个游戏内界面的默认主题渲染（文本框/选择支/存档/设置/历史/画廊/音乐/标题/过场等），统一色板+字体+间距系统 |
| T2-021 | theme.toml 加载与解析 | `aster-ui` | REQ-UI-002 | 8h | Theme 结构体，theme.toml → 完整配置，配置错误时回退默认值 + 给出明确提示 |
| T2-022 | 九宫格（9-Slice）渲染器 | `aster-ui` | REQ-UI-003 | 12h | 9-Slice GPU 渲染（center 拉伸，edge 单向拉伸，corner 不变形），切边值可独立配置 |
| T2-023 | 锚点布局引擎 | `aster-ui` | REQ-UI-004 | 12h | 语义锚点解析（center/bottom/top_left 等 + px 偏移），1280×720 / 1920×1080 / 2560×1440 和 16:9/16:10/21:9 下 UI 不越界 |
| T2-024 | UI 动画引擎 | `aster-ui` | REQ-UI-005 | 12h | spring + cubic-bezier 曲线，slide/fade/scale/stagger/pulse/ripple/ken_burns 效果，60fps 无掉帧 |

### 7.3 任务 — IDE 增强（Week 34-38）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T2-050 | 实时预览面板 | `ide/` | REQ-IDE-030 | 12h | IDE 内嵌预览区域（或独立窗口），保存 .aster 后自动刷新（< 2s），预览不可交互仅展示 |
| T2-051 | 角色管理器 | `ide/` | REQ-IDE-031 | 10h | `.asterchar` CRUD 表单 + 精灵缩略图网格 + 引用该角色的场景列表 |
| T2-052 | 音频试听 | `ide/` | REQ-IDE-032 | 6h | 资源面板中点击音频文件试听，播放/暂停/停止，不重叠播放 |
| T2-053 | 分支流程图 | `ide/` | REQ-IDE-033 | 12h | @vue-flow 渲染有向图，节点可点击跳转脚本，孤立节点高亮警告，死循环检测 |
| T2-054 | 主题可视化编辑器 | `ide/` | REQ-IDE-034 | 12h | 表单式 theme.toml 编辑（色板取色器 + 文件选择器 + 数值滑块），预览窗口实时反映修改 |
| T2-055 | 存档浏览器 | `ide/` | REQ-IDE-035 | 4h | 列表视图（缩略图+时间+场景名），删除确认 |
| T2-056 | 错误内联报告增强 | `ide/` | REQ-IDE-036 | 6h | Warning 级别黄色波浪线，悬停详情，Problems 面板聚合+点击跳转 |

**Phase 5 产出物检查**：
- [ ] 默认主题覆盖全部 12 个游戏内界面，60fps GPU Shader 驱动
- [ ] theme.toml 所有配置项正确生效，配置错误有明确回退提示
- [ ] 9-Slice 贴图在 3 档分辨率下边角不变形
- [ ] 锚点布局在 3 种宽高比下 UI 不越界
- [ ] UI 动画 60fps 无掉帧
- [ ] IDE 实时预览：保存 .aster → 预览刷新 < 2s
- [ ] 分支流程图：节点可点击跳转，孤立节点/死循环可检测
- [ ] 主题编辑器：取色器/文件选择器/滑块正常工作

---

## 8. Phase 6 — 高级渲染与生态分发（Week 39-52，14 周）

### 8.1 目标

完成 Asterism 从专业引擎到完整商业级产品的最后一步。引入 Live2D 骨骼动画、GPU 粒子系统、高级 Shader 后期处理、视频播放。构建分发生态：资源归档打包、WASM 插件系统、Steam 集成、多语言框架。IDE 获得可视化时间线编辑器、脚本调试器和发布向导。撰写完整用户文档和示例游戏。

| 维度 | 范围 |
|------|------|
| **新增 Crate** | `aster-plugin`(wasm host), `aster-steam`(feature flag) |
| **覆盖的 P2 需求** | REQ-ENG-100~105, REQ-ENG-110~113, REQ-IDE-050~054 |

### 8.2 任务 — 高级渲染与视觉（Week 39-44）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T3-001 | 视频播放支持 | `aster-renderer` | REQ-ENG-101 | 16h | WebM/H.264 解码→wgpu 纹理，音频路由到 kira，字幕叠加 |
| T3-002 | Live2D 模型加载 | `aster-renderer` + `aster-asset` | REQ-ENG-100 | 24h | .moc3 加载，纹理图集，网格变形渲染 |
| T3-003 | Live2D 参数动画 | `aster-renderer` | REQ-ENG-100 | 16h | 参数插值，口型/眨眼/呼吸驱动，鼠标追踪 |
| T3-004 | 口型同步（Lip-Sync） | `aster-audio` + `aster-renderer` | REQ-ENG-102 | 16h | 语音波形分析→viseme 序列→驱动 Live2D 参数或精灵切换 |
| T3-005 | 角色空闲动画 | `aster-renderer` | REQ-ENG-103 | 8h | 呼吸微动 / 眨眼定时 / 姿态微调 |
| T3-006 | 高级 Shader 后期 | `aster-renderer` | REQ-ENG-104 | 20h | Gaussian blur / bloom / 3D LUT color grading / vignette / chromatic aberration |
| T3-007 | 粒子系统 | `aster-renderer` | REQ-ENG-105 | 16h | Compute shader 粒子，樱花/雪花/萤火虫预设，自定义发射器配置（.toml） |

### 8.3 任务 — 分发与生态（Week 44-48）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T3-010 | 资源归档打包 | `packager/` | REQ-ENG-110 | 16h | .asterarchive 格式（ZIP + 索引 + AES-256-GCM 加密），引擎透明读取 |
| T3-011 | WASM 插件系统 | 新 `aster-plugin` crate | REQ-ENG-111 | 24h | Wasmtime 宿主，8 个事件钩子，manifest 能力声明，沙箱最小权限 |
| T3-012 | Steam 集成 | 新 `aster-steam` crate（feature flag） | REQ-ENG-112 | 16h | steamworks-rs 绑定，成就解锁/云存档同步/Rich Presence |
| T3-013 | 多语言 / i18n 框架 | `aster-core` + `aster-vm` | REQ-ENG-113 | 16h | 字符串表系统（locale/*.toml），.aster 引用键，字体回退链 |

### 8.4 任务 — IDE 高级功能（Week 47-50）

| 编号 | 任务 | 对应模块 | 对应需求 | 预估 | 交付物 |
|------|------|---------|---------|------|--------|
| T3-020 | 可视化时间线编辑器 | `ide/` | REQ-IDE-050 | 24h | 拖拽节点面板（对话/选择/Show/Hide/Music 等），场景节点图，属性检查器，自动布局 |
| T3-021 | 翻译工具 | `ide/` | REQ-IDE-051 | 12h | 字符串提取，双语并列编辑（源→目标），缺失翻译检测 |
| T3-022 | 资源优化管线 | `ide/` + `packager/` | REQ-IDE-052 | 12h | PNG→basis/BC7，音频→Opus，EXIF 剥离，超大贴图下采样 |
| T3-023 | 脚本调试器 | `ide/` + `aster-vm` | REQ-IDE-053 | 16h | 断点设置，变量监视窗口，单步执行（Step Over/Into），调用栈查看 |
| T3-024 | 发布向导 | `ide/` | REQ-IDE-054 | 12h | 引导式多步骤流程：选平台→设置版本号→配置 Steam App ID→生成安装包→可选上传 itch.io |

### 8.5 任务 — 文档与示例（Week 50-52）

| 编号 | 任务 | 说明 | 预估 | 交付物 |
|------|------|------|------|--------|
| T3-030 | 用户手册 | 从零到发布游戏的完整教程（5 章）：快速开始、脚本编写、素材准备、UI 定制、发布分发 | 20h | `docs/manual/` |
| T3-031 | .aster 语言参考（终稿） | 完整语法规范 + 所有内建函数 + 代码示例 + 从 T1-070 初稿扩展 | 12h | `docs/aster-lang-reference.md` |
| T3-032 | 示例游戏 | 中等规模示范作品（3 章、3 角色、CG/语音/BGM、多结局） | 20h | GitHub 独立仓库 |
| T3-033 | 贡献指南 | CONTRIBUTING.md + 代码贡献流程 + 设计文档索引 | 4h | CONTRIBUTING.md |
| T3-034 | API 文档 | 所有公开 Rust API 的完整文档（cargo doc） + IDE Tauri Commands 文档 | 8h | `docs/api/` |

**Phase 6 产出物检查**：
- [ ] 所有 P2 需求通过验收
- [ ] Live2D 模型正常加载和渲染，口型同步可用
- [ ] 粒子系统 60fps（1000 粒子情况下）
- [ ] 资源归档打包：.asterarchive 加密/解密正确，引擎透明读取
- [ ] WASM 插件：可加载一个示例插件并在事件钩子触发时执行
- [ ] Steam 成就和云存档功能正常（通过 Steamworks 沙箱测试）
- [ ] i18n：至少支持中/英/日三种语言的字符串表
- [ ] 可视化时间线编辑器可拖拽构建一个完整场景
- [ ] 脚本调试器：断点/单步/变量监视可用
- [ ] 用户手册 5 章完整
- [ ] 示例游戏可独立下载并运行
- [ ] 发布 v1.0.0-stable 到 GitHub Releases
- [ ] `cargo doc` 生成完整 API 文档

---

## 9. 风险与依赖

### 9.1 关键技术风险

| 编号 | 风险 | 影响 | 概率 | 影响 Phase | 缓解措施 |
|------|------|------|------|-----------|---------|
| RISK-001 | wgpu / winit API 不稳定 | 渲染模块需重写 | 中 | P1, P4, P6 | 锁定次要版本，CI 测试预警，抽象 Renderer trait 降低耦合 |
| RISK-002 | Tauri v2 大版本升级 | IDE 前端需调整 | 中 | P3, P5 | 锁定版本，升级前在 feature branch 上验证 |
| RISK-003 | cosmic-text 字形布局问题 | CJK 文本渲染 bug | 低 | P1 | 早期建立 CJK 测试用例集，备份方案：swash + 自研布局 |
| RISK-004 | kira 维护停滞 | 音频系统需要替代方案 | 低 | P2 | kira 代码量适中可 fork；备份方案 cpal + rodio |
| RISK-005 | .aster DSL 设计缺陷 | 需要重新设计语法 | 中 | P4 | P1-P3 期间保持语言功能最小化，收集早期用户反馈后再扩展 |
| RISK-006 | 编译时间过长 | 开发效率降低 | 高 | 全部 | rust-cache（CI）+ sccache（本地可选）+ debug opt-level=1（T0-014/015） |
| RISK-007 | 跨平台渲染差异 | 某平台显示异常 | 中 | P1, P4 | CI 中在 3 个平台上运行截图对比测试 |
| RISK-008 | Live2D Cubism SDK 许可 | 法律/合规风险 | 低 | P6 | 使用社区逆向工程的开源加载器（如 live2d-rs），不捆绑官方 SDK |

### 9.2 外部依赖

| 编号 | 依赖 | 说明 | 管理方式 |
|------|------|------|---------|
| DEP-001 | Rust 编译器 | 需要 1.95+ | CI 中锁定 `rust-toolchain.toml` |
| DEP-002 | Node.js | IDE 前端需要 20.x+ | 文档中说明，CI 中指定版本 |
| DEP-003 | wgpu 生态 | 渲染管线基石 | 跟踪 wgpu release notes，参与社区 |
| DEP-004 | Tauri 生态 | IDE 框架基石 | 跟踪 Tauri 发布，报告 bug |
| DEP-005 | Monaco Editor | 脚本编辑器核心 | 版本锁定，定期评估是否需要更新 |

---

## 10. 迭代机制

### 10.1 Phase 评估流程

每个 Phase 结束时执行：

1. **自检清单**：检查该 Phase 的所有交付物和验收标准是否完成
2. **Bug 清零**：该 Phase 产生的所有已知 Bug 必须修复后才能开始下一 Phase
3. **性能回顾**：对照 NFR-PERF 目标值测量，超标项列入下一 Phase 的待办
4. **文档审查**：对照 NFR-MAIN-006 确认架构文档与实际代码一致
5. **Velocity 校准**：根据本 Phase 的实际速度（实际工时 / 预估工时），调整后续 Phase 的预估
6. **范围确认**：如果某 Phase 落后计划 > 30%，评估是削减范围还是延长工期，更新本路线图

### 10.2 任务粒度

- 单个任务不超过 24 小时预估工时
- 超大型任务（如"默认精美主题实现"）作为 Epic 管理，创建子任务
- 每个任务对应一个独立的 Git feature branch（`feat/T2-020-default-theme`）
- 任务状态在 Roadmap.md 中用 checkbox 追踪

### 10.3 分支管理

```
main                     ← 始终可构建，已发布版本
  ├─ develop             ← 日常开发集成分支
  │   ├─ feat/T1-001     ← Phase 1 功能分支
  │   ├─ feat/T2-001     ← Phase 4 功能分支
  │   └─ ...
  ├─ release/v0.1.0      ← Phase 1 发布分支
  ├─ release/v0.5.0      ← Phase 5 发布分支
  └─ hotfix/xxx          ← 紧急修复
```

### 10.4 纯 Vibe Coding 迭代节奏

由于本项目为纯 AI 开发，无人参与 Review 或 Coding，迭代节奏由 AI 自主控制：

- 每次开发会话聚焦一个或多个关联任务
- 完成任务后更新 Roadmap.md 中的 checkbox 状态
- 每个 Phase 内的任务不强制严格顺序，但优先完成被依赖的任务（如 parser 先于 compiler）
- 每次会话结束时检查：测试全部通过、无 clippy warning、文档已更新

---

*本文档定义了 Asterism 的完整开发路线图。这是活文档——随项目推进持续更新。*
