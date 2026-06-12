---
name: "🐛 Bug 报告"
description: "报告引擎、IDE 或 DSL 中的缺陷"
title: "[Bug]: "
labels: ["bug", "needs-triage"]
body:
  - type: markdown
    attributes:
      value: |
        ## 🐛 Bug 报告

        感谢你花时间报告这个问题！请填写以下信息帮助我们快速定位和修复。

  - type: textarea
    id: description
    attributes:
      label: "📝 问题描述"
      description: "简洁清晰地描述发生了什么问题"
      placeholder: "当我在脚本编辑器中按下 Ctrl+S 时，IDE 偶发闪退..."
    validations:
      required: true

  - type: textarea
    id: steps
    attributes:
      label: "🔄 复现步骤"
      description: "按顺序列出触发 Bug 的操作步骤"
      placeholder: |
        1. 打开 Asterism IDE
        2. 新建或打开一个 .aster 脚本
        3. 连续快速按下 Ctrl+S 3-5 次
        4. 观察 IDE 窗口
    validations:
      required: true

  - type: textarea
    id: expected
    attributes:
      label: "✅ 期望行为"
      description: "你期望发生什么"
      placeholder: "每次按下 Ctrl+S 应该触发保存，IDE 保持稳定运行"
    validations:
      required: true

  - type: textarea
    id: actual
    attributes:
      label: "❌ 实际行为"
      description: "实际发生了什么"
      placeholder: "连续快速保存后 IDE 窗口消失，无错误提示"
    validations:
      required: true

  - type: input
    id: screenshot
    attributes:
      label: "📸 截图链接（可选）"
      description: "如果能提供截图或录屏，将大大加速问题定位"
      placeholder: "https://imgur.com/..."

  - type: textarea
    id: environment
    attributes:
      label: "💻 环境信息"
      description: "请提供尽可能多的环境信息"
      value: |
        - 操作系统：Windows 11 24H2
        - Asterism 版本：v0.1.0-pre-alpha
        - 安装方式：源码编译 / pre-built binary
        - 其他相关软件或硬件信息：
    validations:
      required: true

  - type: checkboxes
    id: confirmations
    attributes:
      label: "📋 提交前确认"
      options:
        - label: "我已搜索过现有 Issues，确认这不是重复报告"
          required: true
        - label: "我已提供了复现所需的最小化 `.aster` 脚本（如适用）"
          required: false
