# 🆘 获取帮助

> 使用 Asterism（群星）引擎或 IDE 时遇到问题？我们来帮你。

---

## 📖 文档

在提问之前，请先查阅项目文档——你的问题可能已有答案。

| 文档 | 内容 | 适合场景 |
|------|------|---------|
| [README](./README.md) | 项目概述、快速开始、技术栈 | 了解项目基本情况 |
| [Solution.md](./docs/Solution.md) | 为什么做这个项目、目标用户、价值主张 | 理解项目愿景和边界 |
| [Requirements.md](./docs/Requirements.md) | 完整功能需求（P0/P1/P2） | 确认某个功能是否在计划内 |
| [Architecture.md](./docs/Architecture.md) | 技术架构、crate 设计、数据模型 | 理解引擎内部结构和 API |
| [Roadmap.md](./docs/Roadmap.md) | 开发路线图、里程碑、任务拆分 | 了解功能开发进度 |
| [CLAUDE.md](./.claude/CLAUDE.md) | AI 编码规范、Git 规范 | 贡献代码时的规范参考 |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | 贡献指南 | 如何参与项目 |

---

## 💬 社区支持

### GitHub Discussions

[GitHub Discussions](https://github.com/asterism-engine/asterism/discussions) 是获取社区帮助的最佳场所。你可以：

- 🙋 **Q&A** — 提问关于引擎/IDE 使用的问题
- 💡 **Ideas** — 分享你的创意和建议
- 🎨 **Show & Tell** — 展示你用 Asterism 制作的作品
- 🌐 **General** — 任何与 Asterism 相关的讨论

> **提问技巧**：
> 1. 使用清晰具体的标题（如"如何实现角色立绘的渐变消失效果？"而非"求助！！"）
> 2. 描述你尝试过的方案和遇到的问题
> 3. 附上相关的 `.aster` 脚本片段（如适用）
> 4. 标注你使用的版本和平台

### GitHub Issues

[GitHub Issues](https://github.com/asterism-engine/asterism/issues) 用于：

- 🐛 **Bug 报告** — 你确定发现了 Bug（非使用问题）
- ✨ **功能请求** — 提议新功能

> ⚠️ 请勿在 Issues 中提问使用问题。使用 Discussions 的 Q&A 分类。

---

## 🔍 常见问题（FAQ）

### 项目状态

**Q: Asterism 现在可以用了吗？**

A: 项目当前处于 **Phase 0（预开发阶段）**，尚未发布可用的二进制版本。你可以 Star 仓库关注进展，或查看 [Roadmap.md](./docs/Roadmap.md) 了解开发计划。

**Q: 什么时候发布第一个可用的版本？**

A: v0.1.0-alpha 预计在 Phase 1 结束时发布（约 2026 Q4），届时将包含基础的 DSL 解析、字节码 VM 和 GPU 渲染管线。详见 [Roadmap.md](./docs/Roadmap.md)。

### 技术问题

**Q: Asterism 和 Ren'Py 有什么区别？**

A: 详见 [Solution.md 第 4 节](./docs/Solution.md#4-核心价值主张) 中的竞品对比表。核心差异：Asterism 基于 Rust + wgpu（GPU Shader 驱动 UI），提供原生 Tauri IDE。

**Q: .aster DSL 是什么语言？我能用 Python 写脚本吗？**

A: `.aster` 是 Asterism 自定义的领域特定语言（DSL），语法针对视觉小说场景优化，贴近自然故事描述。不支持 Python 脚本——这是刻意的设计决策：DSL 支持编译期静态分析和更好的错误提示。

**Q: Asterism 支持移动端吗？**

A: MVP（v1.0.0）仅支持桌面平台（Windows / macOS / Linux）。移动端支持是 v1.0.0 之后的长期规划。

**Q: 我能把用 Asterism 做的游戏发布到 Steam 吗？**

A: 当然可以！Asterism 以 MIT 许可发布，你用 Asterism 制作的游戏完全属于你。Steam 集成（成就/云存档）计划在 v1.0.0 中实现。

### 贡献相关

**Q: 我不会 Rust，能参与贡献吗？**

A: 当然！除了 Rust 代码，还有很多贡献方式：改进文档、设计 UI 主题、撰写教程、报告 Bug、翻译界面文本等。详见 [CONTRIBUTING.md](./CONTRIBUTING.md)。

**Q: 项目为什么是"纯 Vibe Coding"？**

A: Asterism 的所有代码由 AI（Claude）独立编写，严格遵循 [CLAUDE.md](./.claude/CLAUDE.md) 中定义的编码规范。这种方式确保了一致的代码风格和高效率的开发节奏。

---

## 🎓 教程资源

> 🚧 教程资源将在 v0.1.0-alpha 发布后陆续上线。

规划中的资源：

- 📝 **快速入门教程** — 30 分钟内制作你的第一部视觉小说
- 📘 **.aster DSL 语言参考** — 完整语法文档
- 🎨 **主题制作指南** — 从默认主题到自定义 UI 的完整教学
- 📦 **项目模板** — 预配置的示例项目（校园恋爱 / 悬疑推理 / 日常喜剧）
- 🎬 **视频教程系列** — 引擎安装、脚本编写、发布全流程

---

## 📧 联系

| 渠道 | 用途 |
|------|------|
| GitHub Issues | Bug 报告、功能请求 |
| GitHub Discussions | 使用问题、社区交流 |
| 邮箱（未来） | 安全漏洞报告: `security@asterism.dev` |

---

> 💡 **提示**：大多数问题的答案都可以在 `docs/` 目录下的文档中找到。花几分钟阅读文档，比等待社区回复更快！
