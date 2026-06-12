/**
 * Asterism IDE — Vue 3 前端
 *
 * 文件路径：ide/src/vite-env.d.ts
 * 功能概述：Vite 环境类型声明 — 为 .vue 单文件组件提供 TypeScript 类型支持
 * 作者：Claude (AI)
 * 创建日期：2026-06-13
 * 最后修改：2026-06-13
 */

/// <reference types="vite/client" />

// 声明 .vue 单文件组件的模块类型
declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<object, object, unknown>;
  export default component;
}
