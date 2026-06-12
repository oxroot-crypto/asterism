/**
 * Asterism IDE — Vite 构建配置
 *
 * 文件路径：ide/vite.config.ts
 * 功能概述：Vite 构建工具配置 — 含 Vue 3 插件、Vitest 测试配置、
 *           Tauri 开发服务器端口锁定、CSP 头配置
 * 作者：Claude (AI)
 * 创建日期：2026-06-13
 * 最后修改：2026-06-13
 */

/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue()],

  // 阻止 Vite 遮盖 Rust 编译错误输出
  clearScreen: false,

  // Tauri v2 期望 dev server 在固定端口
  server: {
    port: 5173,
    strictPort: true,
    // 监听 src-tauri/ 变更由 Tauri 处理，Vite 无需关心
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  // Vitest 配置
  test: {
    // jsdom 模拟浏览器 DOM 环境
    environment: "jsdom",
    // 全局测试 API（describe/it/expect 无需手动导入）
    globals: true,
  },
});
