# Asterism Pull Request 模板

## 📝 变更说明

<!-- 请在此简要描述本 PR 做了什么 -->

- **类型**: <!-- feat / fix / docs / style / refactor / perf / test / chore -->
- **范围**: <!-- core / parser / compiler / vm / renderer / audio / ui / asset / save / runtime / platform / ide / packager -->
- **关联 Issue**: Closes #<!-- 如无则填 N/A -->

## ✅ 自检清单

<!-- 请确认以下每项，已完成的勾选 [x] -->

### Rust 代码质量

- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过（零 warning）
- [ ] `cargo test --workspace` 通过

### IDE 前端代码质量

- [ ] `pnpm --dir ide typecheck` 通过
- [ ] `pnpm --dir ide lint` 通过

### 注释与文档

- [ ] 新增的公开函数/类型有完整的中文 docstring / JSDoc
- [ ] 文件头注释包含功能概述、作者、日期

### 代码质量

- [ ] 没有遗留的 `unwrap()` / `expect()` / `todo!()` / `unimplemented!()`（除非有明确的 Issue 追踪）
- [ ] 没有硬编码的魔法数字（均已定义为命名常量）
- [ ] 没有硬编码的敏感信息（密钥、密码、Token）

### 文档同步

- [ ] 如有架构变更，已同步更新 Architecture.md
- [ ] 如有需求变更，已同步更新 Requirements.md

## 🧪 测试结果

<!-- 请附上本地测试运行结果 -->

<details>
<summary>cargo test --workspace 输出</summary>

```
（粘贴输出）
```

</details>

<details>
<summary>pnpm --dir ide typecheck && pnpm --dir ide test 输出</summary>

```
（粘贴输出）
```

</details>

## 📸 截图 / 录屏（如适用）

<!-- 如果 PR 涉及 UI 变更，请附上截图或录屏 -->

---

> 🤖 本 PR 遵循 Asterism 项目的 CLAUDE.md 编码规范和 Git 规范。
> 如有疑问，请参考 `.claude/CLAUDE.md`。
