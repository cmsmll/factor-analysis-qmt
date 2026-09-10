<script setup lang="ts">
// 看板过滤条容器:白卡 + 统一 flex 排布;过滤项由页面通过插槽提供,
// 槽内统一约定类名:.filter-item / .filter-item__label / .selector-button / .reload-*。
</script>

<template>
  <div class="board-filter-bar">
    <div class="board-filter-form">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.board-filter-bar {
  background: var(--ui-bg-card, #fff);
  padding: 16px 20px;
  border-radius: 8px;
  box-shadow: var(--ui-shadow-card, 0 1px 3px rgb(0 0 0 / 6%));
}

.board-filter-form {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 20px;
}

.board-filter-form :slotted(.filter-item) {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.board-filter-form :slotted(.filter-item__label) {
  flex-shrink: 0;
  font-size: var(--ui-font-base, 14px);
  color: var(--ui-text-label, #1f2225);
}

.board-filter-form :slotted(.selector-button) {
  min-width: 112px;
  color: var(--ui-color-primary, #409eff);
}

.board-filter-form :slotted(.reload-form-item) {
  margin-left: auto;
}

.board-filter-form :slotted(.reload-btn) {
  min-width: 76px;
}

.board-filter-form :slotted(.reload-icon) {
  width: 14px;
  height: 14px;
  filter: brightness(0) invert(1);
}
</style>
