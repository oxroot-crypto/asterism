# 📋 变更日志

> Asterism（群星）项目的所有重要变更记录。
>
> 格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
> 版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

---

## [未发布]

> 当前开发分支 `main` 上的变更。

### Added

- 项目仓库初始化：README、文档体系（Solution / Requirements / Architecture / Roadmap）
- AI 开发行为宪章（`.claude/CLAUDE.md`）— 编码规范、Git 规范、注释规范
- CI/CD 工作流配置（Rust CI / 前端 CI / Release CI）
- Issue 模板（Bug Report / Feature Request）和 PR 模板
- 开源社区标准文件（CONTRIBUTING.md / CODE_OF_CONDUCT.md / SECURITY.md / SUPPORT.md）
- MIT 开源许可证

---

## 版本规划

| 版本 | 阶段 | 预计日期 | 核心内容 |
|------|------|---------|---------|
| v0.0.1-dev | Phase 0 | 2026 Q3 | 项目初始化、CI/CD、规范建立 |
| v0.1.0-alpha | Phase 1 | 2026 Q4 | 引擎基础：DSL 解析、字节码 VM、GPU 渲染管线 |
| v0.2.0-alpha | Phase 2 | 2027 Q1 | 引擎完整：音频系统、资源管理、存档系统 |
| v0.3.0-beta | Phase 3 | 2027 Q2 | IDE MVP：脚本编辑、资源导入、一键构建 |
| v0.4.0-beta | Phase 4 | 2027 Q3 | 游戏体验：转场特效、语音、Skip/Backlog、增强 DSL |
| v0.5.0-rc | Phase 5 | 2027 Q4 | UI 主题系统与 IDE 增强 |
| v1.0.0-stable | Phase 6 | 2028 Q1 | 高级渲染 + 生态分发 |

> 📅 以上日期为预估，实际发布时间以 Roadmap.md 为准。
> 详见 [开发路线图](./docs/Roadmap.md)。

---

## 变更类型说明

| 类型 | 说明 |
|------|------|
| **Added** | 新增功能 |
| **Changed** | 现有功能的变更 |
| **Deprecated** | 即将移除的功能 |
| **Removed** | 已移除的功能 |
| **Fixed** | Bug 修复 |
| **Security** | 安全漏洞修复 |

---

> 📝 本文件将在 v0.1.0-alpha 首次发布后开始记录实质性变更。
