<script setup lang="ts">
defineOptions({ name: 'UiCard' })

withDefaults(
  defineProps<{
    title?: string
    size?: 'small' | 'medium' | 'large'
    /** 是否显示边框（默认细边框卡片） */
    bordered?: boolean
  }>(),
  { size: 'medium', bordered: true },
)
</script>

<template>
  <section class="ui-card" :class="[`ui-card--${size}`, { 'is-unbordered': !bordered }]">
    <header v-if="title || $slots.header" class="ui-card__header">
      <slot name="header"><span class="ui-card__title">{{ title }}</span></slot>
    </header>
    <div class="ui-card__body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.ui-card {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  min-width: 0;
  border: 1px solid var(--ui-border-lighter, #ebeef5);
  border-radius: var(--ui-radius-lg, 8px);
  background: var(--ui-bg-card, #fff);
  box-shadow: var(--ui-shadow-card, 0 1px 3px rgb(0 0 0 / 6%));
}

.is-unbordered {
  border-color: transparent;
  box-shadow: none;
}

.ui-card__header {
  display: flex;
  align-items: center;
  border-bottom: 1px solid var(--ui-border-extra-light, #f2f3f5);
  font-weight: 600;
  color: var(--ui-text-main, #303133);
}

.ui-card--small .ui-card__header {
  padding: 10px 14px;
  font-size: var(--ui-font-base, 14px);
}

.ui-card--medium .ui-card__header {
  padding: 14px 18px;
  font-size: var(--ui-font-base, 14px);
}

.ui-card--large .ui-card__header {
  padding: 16px 20px;
  font-size: var(--ui-font-lg, 18px);
}

.ui-card__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ui-card__body {
  min-width: 0;
  flex: 1;
}

.ui-card--small .ui-card__body {
  padding: 12px 14px;
}

.ui-card--medium .ui-card__body {
  padding: 16px 18px;
}

.ui-card--large .ui-card__body {
  padding: 20px;
}
</style>
