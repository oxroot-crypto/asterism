# 🤝 贡献指南

> **Asterism（群星）** 是一个纯 Vibe Coding 开源项目 —— 所有代码由 AI（Claude）独立完成。
> 但这不意味着社区不能参与！以下是你可以为 Asterism 做出贡献的方式。

---

## 目录

- [行为准则](#行为准则)
- [贡献方式](#贡献方式)
- [开发环境搭建](#开发环境搭建)
- [提交 Issue](#提交-issue)
- [提交 Pull Request](#提交-pull-request)
- [编码规范](#编码规范)
- [代码审查流程](#代码审查流程)
- [社区沟通](#社区沟通)

---

## 行为准则

本项目受 [贡献者公约（Contributor Covenant）](./CODE_OF_CONDUCT.md) 约束。参与本项目即表示你同意遵守该准则。请阅读全文以确保你理解哪些行为是可接受的，哪些是不可接受的。

---

## 贡献方式

### 我可以怎样贡献？

| 贡献方式 | 说明 | 适合人群 |
|---------|------|---------|
| 🐛 **报告 Bug** | 在 GitHub Issues 提交 Bug Report | 所有人 |
| ✨ **提议功能** | 在 GitHub Issues 提交 Feature Request | 所有人 |
| 📖 **改进文档** | 修正错别字、补充说明、翻译文档 | 所有人 |
| 🎨 **贡献主题** | 设计并分享 UI 主题皮肤 | 设计师、创作者 |
| 📝 **撰写教程** | 编写 .aster DSL 使用教程或示例项目 | 创作者 |
| 💬 **社区支持** | 在 Discussions 回答其他用户的问题 | 所有人 |
| 🔧 **贡献代码** | 修复 Bug、实现新功能 | 开发者 |
| 🌍 **国际化翻译** | 翻译引擎/IDE 界面文本 | 翻译者 |

### 特别说明：关于代码贡献

Asterism 是**纯 Vibe Coding** 项目，核心代码由 AI 独立生成。但这并不意味着人类开发者不能贡献代码。我们欢迎：

- **Bug 修复**：如果你发现了引擎或 IDE 中的 Bug，欢迎提交修复
- **文档改进**：文档总是需要更多人来完善
- **主题和皮肤**：`theme.toml` 配置和 9-Slice 贴图不需要深入理解引擎内部
- **工具脚本**：辅助构建、测试、发布的外部脚本
- **测试用例**：为现有功能补充测试用例

对于大规模的功能开发，建议先开 Issue 或 Discussion 讨论设计方案，避免重复劳动。

---

## 开发环境搭建

### 前置依赖

| 工具 | 最低版本 | 说明 |
|------|---------|------|
| Rust | 1.95.0 | 引擎编译 |
| Node.js | 20.x | IDE 前端开发 |
| pnpm | 9.x | 前端包管理 |
| Git | 2.40 | 版本控制 |

### 克隆并构建

```bash
# 克隆仓库
git clone https://github.com/asterism-engine/asterism.git
cd asterism

# 构建全部 Rust 项目（根 Cargo.toml 为 workspace 入口）
cargo build --workspace

# 启动 IDE（开发模式）
cd ide
pnpm install
pnpm tauri dev

# 运行完整测试套件（回到项目根目录）
cd ..
cargo test --workspace
pnpm --dir ide test
```

### 项目结构概览

```
asterism/
├── engine/                    # Rust 引擎 workspace
│   ├── aster-core/            # 核心抽象层（AssetId, VariableStore, Scene）
│   ├── aster-parser/          # .aster DSL 解析器（pest PEG 语法）
│   ├── aster-compiler/        # AST → 字节码编译器
│   ├── aster-vm/              # 字节码虚拟机（指令执行器）
│   ├── aster-renderer/        # wgpu 2D 渲染管线
│   ├── aster-audio/           # 音频系统（kira）
│   ├── aster-ui/              # GPU Shader 驱动 UI 控件库
│   ├── aster-asset/           # 资源管理与加载
│   ├── aster-save/            # 存档/读档系统
│   ├── aster-platform/        # 平台抽象层（winit 集成）
│   └── aster-runtime/         # 应用壳（整合所有模块）
├── ide/                       # Tauri v2 + Vue 3 IDE 桌面应用
│   ├── src/                   # Vue 3 TypeScript 前端
│   └── src-tauri/             # Tauri Rust 后端
├── docs/                      # 项目文档
│   ├── Solution.md            # 项目解决方案
│   ├── Requirements.md        # 完整需求文档
│   ├── Architecture.md        # 技术架构设计
│   └── Roadmap.md             # 开发路线图
└── .claude/
    └── CLAUDE.md              # AI 开发行为宪章（编码规范、Git 规范）
```

---

## 提交 Issue

### Bug Report

1. 打开 [New Issue](https://github.com/asterism-engine/asterism/issues/new/choose)
2. 选择 **"🐛 Bug 报告"** 模板
3. 填写以下信息：
   - 问题描述（简洁清晰）
   - 复现步骤（按顺序列出操作）
   - 期望行为和实际行为
   - 环境信息（操作系统、版本号、安装方式）
4. 在提交前搜索已有 Issues，避免重复

> **高质量 Bug Report 的特征**：
> - 包含可复现的最小化 `.aster` 脚本
> - 包含截图或录屏
> - 填写了完整的系统信息

### Feature Request

1. 打开 [New Issue](https://github.com/asterism-engine/asterism/issues/new/choose)
2. 选择 **"✨ 功能请求"** 模板
3. 描述功能、使用场景和建议方案
4. 确认此功能与 Asterism 的产品定位一致

> **重要的前置阅读**：[Requirements.md](./docs/Requirements.md) 已列出了计划中的功能。请先确认你的需求不在已有的 P0/P1/P2 需求列表中。

---

## 提交 Pull Request

### PR 工作流

```bash
# 1. Fork 本仓库（通过 GitHub Web UI）

# 2. 克隆你的 Fork
git clone https://github.com/<your-username>/asterism.git
cd asterism

# 3. 添加上游仓库
git remote add upstream https://github.com/asterism-engine/asterism.git

# 4. 创建功能分支
# 分支命名：feat/<描述> / fix/<描述> / docs/<描述>
git checkout -b fix/save-thumbnail-crash

# 5. 进行修改并提交
# Commit 格式：<type>(<scope>): <中文描述>
git add .
git commit -m "fix(save): 修复存档缩略图在 4K 分辨率下的崩溃问题"

# 6. 同步上游变更
git fetch upstream
git rebase upstream/main

# 7. 推送到你的 Fork
git push origin fix/save-thumbnail-crash

# 8. 在 GitHub 上创建 Pull Request
```

### PR 标题格式

遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <中文描述>
```

**Type 必须为以下之一**：

| Type | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 文档变更 |
| `style` | 代码格式（不影响功能） |
| `refactor` | 代码重构 |
| `perf` | 性能优化 |
| `test` | 测试相关 |
| `chore` | 构建/工具/依赖 |

**Scope 为模块名称**：`core`, `parser`, `compiler`, `vm`, `renderer`, `audio`, `ui`, `asset`, `save`, `runtime`, `platform`, `ide`, `packager`

### PR 提交前自检

在提交 PR 前，请确保（全部从项目根目录运行）：

- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过（零 warning）
- [ ] `cargo test --workspace` 通过
- [ ] `pnpm --dir ide typecheck` 通过
- [ ] `pnpm --dir ide lint` 通过
- [ ] 新增的公开函数/类型有完整的中文 docstring / JSDoc
- [ ] 没有遗留的 `unwrap()` / `expect()` / `todo!()` / `unimplemented!()`
- [ ] 没有硬编码的魔法数字
- [ ] 如有架构变更，已同步更新 `docs/Architecture.md`

---

## 编码规范

Asterism 项目有严格的编码规范，定义在 [.claude/CLAUDE.md](./.claude/CLAUDE.md) 中。所有代码贡献者必须遵循该规范。

### 关键原则

1. **注释必须详细、完整、规范** — 每个源文件头部包含文件级注释，每个公开 API 包含完整 docstring
2. **注释语言使用简体中文** — 包括文档、注释、Commit Message
3. **禁止使用 `unwrap()` / `expect()` 在非测试代码中** — 使用 `?` 或 `match` 传播错误
4. **TypeScript strict mode** — 禁止使用 `any` 类型
5. **零 warning 策略** — 从项目根目录运行 `cargo clippy --workspace --all-targets -- -D warnings` 和 `pnpm lint` 必须零 warning 通过

### 项目文档结构

在开始任何代码贡献前，建议先阅读以下文档以理解项目全貌：

| 阅读顺序 | 文档 | 内容 |
|---------|------|------|
| 1 | [Solution.md](./docs/Solution.md) | 为什么做这个项目、目标用户、价值主张 |
| 2 | [Requirements.md](./docs/Requirements.md) | 完整功能需求（P0/P1/P2）、非功能需求 |
| 3 | [Architecture.md](./docs/Architecture.md) | 技术架构、crate 设计、渲染管线 |
| 4 | [Roadmap.md](./docs/Roadmap.md) | 开发路线图、里程碑、任务拆分 |
| 5 | [CLAUDE.md](./.claude/CLAUDE.md) | 编码规范、Git 规范、AI 行为准则 |

---

## 代码审查流程

由于 Asterism 是纯 Vibe Coding 项目（代码由 AI 生成），人类提交的 PR 将由 AI 进行代码审查：

1. PR 提交后，CI 自动化检查会立即运行（`cargo fmt` / `cargo clippy` / `cargo test` / `pnpm lint` 等，全部从项目根目录执行）
2. AI 审查者会检查代码是否符合 CLAUDE.md 定义的编码规范
3. 审查意见通过 PR Comments 给出
4. 修改完成后，审查通过则合并

> 审查的重点是编码规范合规性、测试覆盖率和文档完整性，而非代码风格偏好。

---

## 社区沟通

| 渠道 | 用途 |
|------|------|
| [GitHub Issues](https://github.com/asterism-engine/asterism/issues) | Bug 报告、功能请求 |
| [GitHub Discussions](https://github.com/asterism-engine/asterism/discussions) | 技术讨论、社区交流、问题求助 |
| [GitHub Wiki](https://github.com/asterism-engine/asterism/wiki) | 教程、FAQ、最佳实践（规划中） |

### 沟通礼仪

- 在 Issues 中保持专业和尊重
- 提问前先搜索是否有重复问题
- 使用清晰、具体的标题
- 中文或英文均可，但建议使用你更擅长的语言确保表达准确

---

## 许可

贡献给 Asterism 的代码以 [MIT](./LICENSE) 许可发布。提交 PR 即表示你同意在该许可下发布你的贡献。

---

> 🌌 **让每一颗故事之星都能闪耀。** —— Asterism 团队
