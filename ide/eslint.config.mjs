/**
 * Asterism IDE — ESLint 配置
 *
 * 文件路径：ide/eslint.config.mjs
 * 功能概述：ESLint 9.x flat config — 集成 Vue 3 + TypeScript 规则，
 *           确保代码风格符合 CLAUDE.md §2.2 编码规范
 * 作者：Claude (AI)
 * 创建日期：2026-06-13
 * 最后修改：2026-06-13
 */
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";

export default [
  // 忽略目录
  {
    ignores: ["dist/**", "src-tauri/**", "node_modules/**"],
  },

  // 基础 JavaScript 推荐规则
  js.configs.recommended,

  // TypeScript 推荐规则
  ...tseslint.configs.recommended,

  // Vue 3 推荐规则（flat config 模式）
  ...pluginVue.configs["flat/recommended"],

  // 项目级规则覆盖
  {
    files: ["**/*.ts", "**/*.vue"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: [".vue"],
      },
    },
    rules: {
      // 允许单词组件名（如 App.vue）
      "vue/multi-word-component-names": "off",

      // 与 Prettier 协作 — 关闭格式规则
      "vue/html-indent": "off",
      "vue/max-attributes-per-line": "off",
      "vue/html-closing-bracket-newline": "off",
      "vue/singleline-html-element-content-newline": "off",

      // TypeScript 规则调整
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],
    },
  },
];
