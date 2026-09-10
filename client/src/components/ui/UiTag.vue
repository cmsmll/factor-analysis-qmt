<script setup lang="ts">
withDefaults(
  defineProps<{
    size?: 'small' | 'medium'
    /** 是否显示边框（默认不显示，naive 调用处传 bordered:false） */
    bordered?: boolean
    type?: 'default' | 'primary' | 'success' | 'warning' | 'error'
  }>(),
  { size: 'medium', bordered: false, type: 'default' },
)
</script>

<template>
  <span class="ui-tag" :class="[`ui-tag--${size}`, `ui-tag--${type}`, { 'has-border': bordered }]">
    <slot />
  </span>
</template>

<style scoped>
.ui-tag {
  display: inline-flex;
  align-items: center;
  box-sizing: border-box;
  border-radius: var(--ui-radius-sm, 4px);
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
  color: var(--ui-text-regular, #606266);
  background: var(--ui-bg-header, #fafafa);
}

.ui-tag.has-border {
  border: 1px solid var(--ui-border-light, #e4e7ed);
}

.ui-tag--small {
  height: 20px;
  padding: 0 6px;
  font-size: var(--ui-font-xs, 12px);
}

.ui-tag--medium {
  height: 24px;
  padding: 0 8px;
  font-size: var(--ui-font-xs, 12px);
}

.ui-tag--primary {
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary-weak, #ecf5ff);
}

.ui-tag--success {
  color: var(--ui-color-success, #67c23a);
  background: rgb(103 194 58 / 0.12);
}

.ui-tag--warning {
  color: var(--ui-color-warning, #e6a23c);
  background: rgb(230 162 60 / 0.12);
}

.ui-tag--error {
  color: var(--ui-color-error, #f56c6c);
  background: rgb(245 108 108 / 0.12);
}
</style>
