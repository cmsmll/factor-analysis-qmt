<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

defineOptions({ name: 'UiSelect' })

export interface UiSelectOption {
  label: string
  value: string | number
  disabled?: boolean
}

const props = withDefaults(
  defineProps<{
    value?: string | number | null
    options?: UiSelectOption[]
    placeholder?: string
    clearable?: boolean
    disabled?: boolean
    size?: 'small' | 'medium'
    /** 面板最小宽度（缺省跟随触发宽度） */
    width?: number
  }>(),
  { placeholder: '请选择', size: 'medium' },
)

const emit = defineEmits<{
  (e: 'update:value', value: string | number | null): void
}>()

const rootRef = ref<HTMLDivElement | null>(null)
/** 面板经 Teleport 到 body,需单独排除其内部点击(否则选项点击会先触发外部关闭) */
const panelRef = ref<HTMLElement | null>(null)
const open = ref(false)
const panelStyle = ref({ top: '0px', left: '0px', width: '0px' })

const selectedLabel = computed(() => {
  const match = props.options?.find((option) => option.value === props.value)
  return match ? match.label : ''
})

function pick(option: UiSelectOption): void {
  if (option.disabled) return
  // 先收合再通知父级:父级 handler 若触发路由/重建,也不影响面板关闭
  open.value = false
  emit('update:value', option.value)
}

function clear(event: MouseEvent): void {
  event.stopPropagation()
  emit('update:value', null)
}

async function toggle(): Promise<void> {
  if (props.disabled) return
  open.value = !open.value
  if (open.value) {
    await nextTick()
    positionPanel()
  }
}
function positionPanel(): void {
  const root = rootRef.value
  if (!root) return
  const rect = root.getBoundingClientRect()
  const height = Math.min(props.options?.length ? props.options.length * 32 + 12 : 100, 280) + 8
  const spaceBelow = window.innerHeight - rect.bottom
  const openUp = spaceBelow < height && rect.top > spaceBelow
  const top = openUp ? rect.top - height : rect.bottom + 4
  panelStyle.value = {
    top: `${Math.max(4, top)}px`,
    left: `${rect.left}px`,
    width: `${props.width ?? Math.max(rect.width, 120)}px`,
  }
}

function onScrollOrResize(): void {
  if (open.value) positionPanel()
}

function onClickOutside(event: MouseEvent): void {
  const target = event.target as Node
  if (rootRef.value?.contains(target)) return
  if (panelRef.value?.contains(target)) return
  open.value = false
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') open.value = false
}

function bindPanelEvents(): void {
  window.addEventListener('scroll', onScrollOrResize, true)
  window.addEventListener('resize', onScrollOrResize)
  document.addEventListener('mousedown', onClickOutside)
  window.addEventListener('keydown', onKeydown)
}

function unbindPanelEvents(): void {
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
  document.removeEventListener('mousedown', onClickOutside)
  window.removeEventListener('keydown', onKeydown)
}

watch(open, (value) => {
  if (value) bindPanelEvents()
  else unbindPanelEvents()
})

onBeforeUnmount(() => {
  unbindPanelEvents()
})
</script>

<template>
  <div ref="rootRef" class="ui-select" :class="[`ui-select--${size}`, { 'is-disabled': disabled }]">
    <button
      type="button"
      class="ui-select__trigger"
      :disabled="disabled"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="toggle"
    >
      <span class="ui-select__label" :class="{ 'is-placeholder': !selectedLabel }">
        <slot name="selected" :label="selectedLabel">{{ selectedLabel || placeholder }}</slot>
      </span>
      <span
        v-if="clearable && value !== null && value !== undefined"
        class="ui-select__clear"
        role="button"
        aria-label="清空"
        tabindex="-1"
        @click.stop="clear"
      >
        <svg viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8" /></svg>
      </span>
      <svg class="ui-select__arrow" viewBox="0 0 12 8"><path d="M1 1.5L6 6.5L11 1.5" /></svg>
    </button>

    <Teleport to="body">
      <Transition name="ui-select-pop">
        <div v-if="open" ref="panelRef" class="ui-select__panel" :style="panelStyle" role="listbox">
          <button
            v-for="option in options"
            :key="String(option.value)"
            type="button"
            class="ui-select__option"
            :class="{
              'is-active': option.value === value,
              'is-disabled': option.disabled,
            }"
            role="option"
            :aria-selected="option.value === value"
            @click="pick(option)"
          >
            <span class="ui-select__option-text">{{ option.label }}</span>
            <svg v-if="option.value === value" class="ui-select__check" viewBox="0 0 12 10">
              <path d="M1.5 5l3 3 6-7" />
            </svg>
          </button>
          <div v-if="!options || options.length === 0" class="ui-select__empty">无选项</div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.ui-select {
  display: inline-flex;
  position: relative;
  box-sizing: border-box;
  width: 100%;
}

.ui-select__trigger {
  display: flex;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: var(--ui-radius-base, 6px);
  color: var(--ui-text-control, #333234);
  background: #fff;
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.ui-select--small .ui-select__trigger {
  height: 28px;
  font-size: var(--ui-font-xs, 12px);
}

.ui-select--medium .ui-select__trigger {
  height: 34px;
  font-size: var(--ui-font-base, 14px);
}

.ui-select.is-disabled .ui-select__trigger {
  cursor: not-allowed;
  opacity: 0.75;
  background: var(--ui-bg-header, #fafafa);
}

.ui-select__trigger:hover:not(:disabled),
.ui-select__trigger:focus-visible {
  border-color: var(--ui-color-primary, #409eff);
  outline: none;
}

.ui-select__label {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ui-select__label.is-placeholder {
  color: var(--ui-text-placeholder, #c0c4cc);
}

.ui-select__arrow {
  width: 11px;
  height: 8px;
  flex: 0 0 auto;
  fill: none;
  stroke: var(--ui-text-placeholder, #c0c4cc);
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
  transition: transform 160ms ease;
}

.ui-select__trigger[aria-expanded='true'] .ui-select__arrow {
  transform: rotate(180deg);
}

.ui-select__clear {
  display: inline-flex;
  width: 16px;
  height: 16px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: var(--ui-text-placeholder, #c0c4cc);
  background: rgb(0 0 0 / 0.06);
  cursor: pointer;
  padding: 0;
}

.ui-select__clear:hover {
  color: var(--ui-text-regular, #606266);
}

.ui-select__clear svg {
  width: 9px;
  height: 9px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
}

/* 弹层面板（body teleport） */
.ui-select__panel {
  position: fixed;
  z-index: 12000;
  box-sizing: border-box;
  max-height: 280px;
  overflow-y: auto;
  padding: 6px;
  border: 1px solid var(--ui-border-light, #e4e7ed);
  border-radius: var(--ui-radius-base, 6px);
  background: #fff;
  box-shadow: var(--ui-shadow-dropdown, 0 4px 16px rgb(0 0 0 / 12%));
}

.ui-select__option {
  display: flex;
  width: 100%;
  box-sizing: border-box;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 10px;
  border: 0;
  border-radius: var(--ui-radius-sm, 4px);
  color: var(--ui-text-regular, #606266);
  background: transparent;
  cursor: pointer;
  font: inherit;
  font-size: var(--ui-font-sm, 13px);
  text-align: left;
}

.ui-select__option:hover {
  color: var(--ui-text-main, #303133);
  background: var(--ui-bg-hover, #f5f7fa);
}

.ui-select__option.is-active {
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary-weak, #ecf5ff);
}

.ui-select__option.is-disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.ui-select__option-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ui-select__check {
  width: 12px;
  height: 10px;
  flex: 0 0 auto;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.ui-select__empty {
  padding: 18px 0;
  color: var(--ui-text-placeholder, #c0c4cc);
  font-size: var(--ui-font-sm, 13px);
  text-align: center;
}

.ui-select-pop-enter-active,
.ui-select-pop-leave-active {
  transition:
    opacity 140ms ease,
    transform 140ms ease;
}

.ui-select-pop-enter-from,
.ui-select-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
