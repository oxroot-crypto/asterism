/**
 * Asterism IDE — Vue 3 前端
 *
 * 文件路径：ide/src/main.ts
 * 功能概述：Vue 3 应用入口 — 创建 Vue 应用实例、挂载 Pinia Store、
 *           注册全局错误处理器、挂载到 #app 根元素
 * 作者：Claude (AI)
 * 创建日期：2026-06-13
 * 最后修改：2026-06-13
 */

import { createApp } from "vue";
import App from "./App.vue";

// 创建 Vue 应用实例
const app = createApp(App);

// 全局错误处理 — 捕获未处理的组件异常并友好提示
app.config.errorHandler = (err, _instance, info) => {
  console.error(`[Asterism IDE] 全局错误 — ${info}:`, err);
};

// 挂载到 #app
app.mount("#app");
