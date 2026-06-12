---
name: "✨ 功能请求"
description: "提议新功能或改进"
title: "[Feature]: "
labels: ["enhancement", "needs-triage"]
body:
  - type: markdown
    attributes:
      value: |
        ## ✨ 功能请求

        感谢你为 Asterism 贡献创意！请尽可能详细地描述你期望的功能。

  - type: textarea
    id: feature
    attributes:
      label: "🎯 功能描述"
      description: "简洁描述你期望的功能是什么"
      placeholder: "我希望在脚本编辑器中右键可以快速插入 DSL 关键字模板..."
    validations:
      required: true

  - type: textarea
    id: usecase
    attributes:
      label: "🎬 使用场景"
      description: "描述这个功能在什么场景下会被使用，解决什么问题"
      placeholder: "作为视觉小说创作者，每次写 menu 选择支都需要手敲完整语法。如果有右键模板插入，可以节省大量重复输入时间。"
    validations:
      required: true

  - type: textarea
    id: proposal
    attributes:
      label: "💡 建议方案"
      description: "如果你有具体实现想法，请在这里描述"
      placeholder: "在 Monaco Editor 的 context menu 中增加'Asterism DSL 模板'子菜单，包含 menu / dialogue / narration 等常用关键字的模板插入选项。"

  - type: textarea
    id: alternatives
    attributes:
      label: "🔄 替代方案"
      description: "是否有其他方式可以达到类似效果？"
      placeholder: "可以使用 snippet 功能，但在右键菜单中找到会更直观。"

  - type: checkboxes
    id: confirmations
    attributes:
      label: "📋 提交前确认"
      options:
        - label: "我已搜索过现有 Issues，确认这不是重复请求"
          required: true
        - label: "此功能与 Asterism 作为视觉小说创作工具的定位一致"
          required: true
