<!--
Asterism IDE — Vue 3 前端

文件路径：ide/src/App.vue
功能概述：根组件 — 显示 IDE 占位页面，包含应用标题和加载提示。
         后续 Phase 将在本组件中集成路由、全局布局、状态管理。
作者：Claude (AI)
创建日期：2026-06-13
最后修改：2026-06-13
-->
<script setup lang="ts">
// Phase 0: 占位组件，不包含实际 IDE 功能
// Phase 3 将在此集成编辑器面板、资源管理器、预览窗口等
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <h1 class="app-title">🌌 Asterism IDE</h1>
      <p class="app-subtitle">群星 — Galgame/ADV 视觉小说创作工具</p>
    </header>

    <main class="app-main">
      <div class="loading-state">
        <div class="loading-spinner" />
        <p class="loading-text">Loading...</p>
        <p class="loading-hint">引擎初始化中，请稍候</p>
      </div>
    </main>

    <footer class="app-footer">
      <span class="version-text">v0.1.0-pre-alpha</span>
    </footer>
  </div>
</template>

<style scoped>
/* 占位页面样式 — Phase 3 将由完整 IDE 布局替代 */
.app-shell {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  background: #1a1a2e;
  color: #e0e0e0;
  font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
}

.app-header {
  text-align: center;
  padding: 60px 20px 0;
}

.app-title {
  font-size: 2.5rem;
  font-weight: 300;
  margin: 0;
  color: #ffffff;
  letter-spacing: 0.05em;
}

.app-subtitle {
  font-size: 1rem;
  color: #8888aa;
  margin-top: 8px;
}

.app-main {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.loading-state {
  text-align: center;
}

.loading-spinner {
  width: 40px;
  height: 40px;
  border: 3px solid #333355;
  border-top-color: #7c8cf8;
  border-radius: 50%;
  margin: 0 auto 16px;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-text {
  font-size: 1.2rem;
  color: #aaaacc;
  margin: 0;
}

.loading-hint {
  font-size: 0.85rem;
  color: #666688;
  margin: 8px 0 0;
}

.app-footer {
  text-align: center;
  padding: 20px;
}

.version-text {
  font-size: 0.8rem;
  color: #555577;
}
</style>
