<script setup lang="ts">
import { computed } from 'vue'

defineOptions({ name: 'UiTabs' })

export interface UiTabItem {
  name: string | number
  label: string
}

const props = withDefaults(
  defineProps<{
    value?: string | number
    /** segment=分段胶囊；bar=下划线页签 */
    type?: 'segment' | 'bar'
    tabs: UiTabItem[]
    size?: 'small' | 'medium'
  }>(),
  { type: 'bar', size: 'medium' },
)

const emit = defineEmits<{ (e: 'update:value', value: string | number): void }>()

const activeName = computed(() => {
  if (props.value !== undefined && props.tabs.some((tab) => tab.name === props.value)) return props.value
  return props.tabs[0]?.name
})

function select(name: string | number): void {
  if (name === props.value) return
  emit('update:value', name)
}
</script>

<template>
  <nav class="ui-tabs" :class="[`ui-tabs--${type}`, `ui-tabs--${size}`]" role="tablist">
    <button
      v-for="tab in tabs"
      :key="String(tab.name)"
      type="button"
      class="ui-tabs__item"
      :class="{ 'is-active': tab.name === activeName }"
      role="tab"
      :aria-selected="tab.name === activeName"
      @click="select(tab.name)"
    >
      {{ tab.label }}
    </button>
  </nav>
</template>

<style scoped>
.ui-tabs {
  display: inline-flex;
  gap: 4px;
}

.ui-tabs__item {
  box-sizing: border-box;
  border: 0;
  color: var(--ui-text-regular, #606266);
  background: transparent;
  cursor: pointer;
  font: inherit;
  white-space: nowrap;
}

/* bar 型：下划线 */
.ui-tabs--bar .ui-tabs__item {
  position: relative;
  padding: 6px 14px;
  font-size: var(--ui-font-base, 14px);
}

.ui-tabs--bar .ui-tabs__item::after {
  position: absolute;
  right: 10px;
  bottom: 0;
  left: 10px;
  height: 2px;
  border-radius: 2px;
  background: var(--ui-color-primary, #409eff);
  content: '';
  opacity: 0;
}

.ui-tabs--bar .ui-tabs__item.is-active {
  color: var(--ui-color-primary, #409eff);
  font-weight: 600;
}

.ui-tabs--bar .ui-tabs__item.is-active::after {
  opacity: 1;
}

/* segment 型：分段胶囊 */
.ui-tabs--segment .ui-tabs__item {
  height: 30px;
  padding: 0 16px;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  margin-right: -1px;
  font-size: var(--ui-font-sm, 13px);
}

.ui-tabs--segment .ui-tabs__item:first-of-type {
  border-radius: var(--ui-radius-base, 6px) 0 0 var(--ui-radius-base, 6px);
}

.ui-tabs--segment .ui-tabs__item:last-of-type {
  border-radius: 0 var(--ui-radius-base, 6px) var(--ui-radius-base, 6px) 0;
}

.ui-tabs--segment .ui-tabs__item.is-active {
  position: relative;
  z-index: 1;
  border-color: var(--ui-color-primary, #409eff);
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary-weak, #ecf5ff);
}

.ui-tabs--small .ui-tabs__item {
  height: 26px;
  padding: 0 12px;
  font-size: var(--ui-font-xs, 12px);
}
</style>
