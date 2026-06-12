# 🌌 Asterism（群星）

> **让每一颗故事之星都能闪耀。**
>
> 开源、专业级 Galgame/ADV 游戏引擎 + 集成开发环境（IDE）

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-v2-8A2BE2.svg)](https://v2.tauri.app/)
[![Vue](https://img.shields.io/badge/vue-3.x-4FC08D.svg)](https://vuejs.org/)
[![Status](https://img.shields.io/badge/status-pre--alpha-red.svg)]()

---

## 简介

**Asterism**（群星）是一个从剧本编写到最终发布的一站式视觉小说创作工具。它由两部分组成：

| 组件 | 技术 | 说明 |
|------|------|------|
| **Asterism Engine** | Rust + wgpu | GPU 驱动的 2D 视觉小说运行时引擎，支持 Vulkan / DirectX 12 / Metal |
| **Asterism IDE** | Tauri v2 + Vue 3 | 集成脚本编辑、资源管理、实时预览、一键构建的桌面应用 |

### 核心特性

- 🎨 **GPU Shader 驱动 UI** — 原生支持毛玻璃模糊、渐变、发光、粒子、色彩分级等现代视觉效果
- 📝 **声明式 .aster DSL** — 领域优化的脚本语言，语法贴近自然故事描述，支持编译期静态分析
- 🎭 **完整主题系统** — `theme.toml` 声明式配置 + 9-Slice 自适应缩放，定制 UI 无需写代码
- 🚀 **跨平台桌面** — Windows / macOS / Linux 统一支持，引擎二进制 < 50 MB
- 🔧 **一体化工作流** — 从剧本编写 → 素材管理 → 实时预览 → 一键构建安装包，全部在一个应用中完成
- 📦 **开源自由** — MIT许可，完全免费，社区驱动

---

## 快速开始

> **当前状态**：项目处于预开发阶段（Phase 0），尚未发布可用的二进制版本。以下为开发环境搭建指南。

### 前置依赖

| 工具 | 最低版本 | 说明 |
|------|---------|------|
| Rust | 1.95.0 | 引擎编译 |
| Node.js | 20.x | IDE 前端开发 |
| pnpm | 9.x | 前端包管理 |
| Git | 2.40 | 版本控制 |

### 开发环境搭建

```bash
# 克隆仓库
git clone https://github.com/asterism-engine/asterism.git
cd asterism

# 构建引擎（根 Cargo.toml 为 workspace 入口，可直接运行）
cargo build --workspace --release

# 启动 IDE（开发模式）
cd ide
pnpm install
pnpm tauri dev

# 运行完整测试套件
cd ..
cargo test --workspace
pnpm --dir ide test
```

---

## 文档索引

| 文档 | 说明 |
|------|------|
| [📋 Solution.md](./docs/Solution.md) | 项目解决方案：问题陈述、用户画像、竞品对比、价值主张、成功指标 |
| [📝 Requirements.md](./docs/Requirements.md) | 完整需求文档：功能需求（P0/P1/P2）、非功能需求、用户故事 |
| [🏗️ Architecture.md](./docs/Architecture.md) | 技术架构设计：crate 设计、渲染管线、数据模型、接口定义、安全架构 |
| [🗺️ Roadmap.md](./docs/Roadmap.md) | 开发路线图：4 个 Phase 里程碑、任务拆分、风险与依赖 |
| [🤖 CLAUDE.md](./.claude/CLAUDE.md) | AI 开发行为宪章：编码规范、Git 规范、注释规范、会话行为准则 |

---

## 技术栈概览

```
┌─────────────────────────────────────────────────┐
│                  Asterism IDE                    │
│         Tauri v2  •  Vue 3  •  TypeScript       │
│     Monaco Editor  •  VueFlow  •  PrimeVue       │
├─────────────────────────────────────────────────┤
│                Asterism Engine                   │
│                                                 │
│  ┌──────────┐ ┌──────────┐ ┌───────────────┐   │
│  │ .aster   │ │ Bytecode │ │ wgpu 2D       │   │
│  │ Parser   │→│ Compiler │→│ Renderer      │   │
│  │ (pest)   │ │          │ │ (Vulkan/DX12/ │   │
│  │          │ │          │ │  Metal)       │   │
│  └──────────┘ └──────────┘ └───────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌───────────────┐   │
│  │ Bytecode │ │ UI Theme │ │ Audio         │   │
│  │ VM       │ │ System   │ │ (kira)        │   │
│  └──────────┘ └──────────┘ └───────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌───────────────┐   │
│  │ Asset    │ │ Save/Load│ │ Platform      │   │
│  │ Manager  │ │ System   │ │ Abstraction   │   │
│  └──────────┘ └──────────┘ └───────────────┘   │
├─────────────────────────────────────────────────┤
│              Rust Workspace (12 crates)          │
└─────────────────────────────────────────────────┘
```

---

## 开发路线图

| 阶段 | 版本 | 工期 | 核心目标 |
|------|------|------|---------|
| Phase 0 | v0.0.1-dev | 2 周 | 项目初始化、CI/CD、规范建立 |
| Phase 1 | v0.1.0-alpha | 8 周 | 引擎基础：脚本解析与 GPU 渲染管线 |
| Phase 2 | v0.2.0-alpha | 6 周 | 引擎完整：音频、资源管理与存档系统 |
| Phase 3 | v0.3.0-beta | 6 周 | IDE MVP：脚本编辑、资源导入、一键构建 |
| Phase 4 | v0.4.0-beta | 8 周 | 游戏体验：转场特效、语音、Skip/Backlog、增强 DSL |
| Phase 5 | v0.5.0-rc | 8 周 | UI 主题系统与 IDE 增强（流程图/主题编辑器） |
| Phase 6 | v1.0.0-stable | 14 周 | 高级渲染（Live2D/粒子/Shader）+ 生态分发（Steam/WASM/i18n） |

详见 [📋 Roadmap.md](./docs/Roadmap.md)

---

## 贡献

本项目为**纯 Vibe Coding** 开源项目 —— 所有代码由 AI（Claude）独立完成。AI 开发严格遵循 [.claude/CLAUDE.md](./.claude/CLAUDE.md) 中定义的编码规范和行为准则。

欢迎通过以下方式参与：
- 🐛 提交 Bug Report 或 Feature Request（GitHub Issues）
- 📖 改进文档
- 🎨 贡献默认主题或 UI 皮肤
- 📝 撰写 .aster 脚本教程或示例

在提交 PR 前，请确保通过自检清单：从项目根目录运行 `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `pnpm --dir ide typecheck` / `pnpm --dir ide lint`。

更多信息请阅读：
- [🤝 CONTRIBUTING.md](./CONTRIBUTING.md) — 完整贡献指南
- [📜 CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) — 贡献者行为准则
- [🔒 SECURITY.md](./SECURITY.md) — 安全策略与漏洞报告
- [🆘 SUPPORT.md](./SUPPORT.md) — 获取帮助与常见问题

---

## 许可

Asterism 以 [MIT](LICENSE)许可发布。

默认主题使用的字体为开源许可（SIL OFL 等），详见 `fonts/` 目录下的许可文件。

---

<p align="center">
  <sub>✨ 由 Claude (AI) 独立编码 · 纯 Vibe Coding 项目 ✨</sub>
</p>
