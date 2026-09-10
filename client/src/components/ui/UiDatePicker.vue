<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

defineOptions({ name: 'UiDatePicker' })

const props = withDefaults(
  defineProps<{
    /** naive 兼容：毫秒时间戳（本地零点）或 null */
    value?: number | null
    placeholder?: string
    clearable?: boolean
    disabled?: boolean
    size?: 'small' | 'medium'
    /** 显示格式：yyyy-MM-dd（仅年/月/日 token） */
    format?: string
    /** 日历面板底部额外按钮文案（如「复位」），为空不显示 */
    actionText?: string
    /** 最小可选日期（毫秒） */
    min?: number
    /** 最大可选日期（毫秒） */
    max?: number
  }>(),
  { placeholder: '选择日期', size: 'medium', format: 'yyyy-MM-dd', actionText: '' },
)

const emit = defineEmits<{
  (e: 'update:value', value: number | null): void
  /** 点击面板 actionText 按钮（面板不自动关闭，由页面决定是否赋值） */
  (e: 'action'): void
}>()

const rootRef = ref<HTMLDivElement | null>(null)
/** 面板经 Teleport 到 body,需单独排除其内部点击(否则年月钮/日格会被当作外部点击关闭) */
const panelRef = ref<HTMLElement | null>(null)
const open = ref(false)
const panelStyle = ref({ top: '0px', left: '0px' })
/** 面板当前浏览的年月（本地） */
const viewYear = ref(new Date().getFullYear())
const viewMonth = ref(new Date().getMonth()) // 0-based
const weekLabels = ['日', '一', '二', '三', '四', '五', '六']

const displayText = computed(() => {
  if (props.value === null || props.value === undefined) return ''
  const date = new Date(props.value)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (value: number) => String(value).padStart(2, '0')
  return props.format
    .replace('yyyy', String(date.getFullYear()))
    .replace('MM', pad(date.getMonth() + 1))
    .replace('dd', pad(date.getDate()))
})

/** 当前月网格：6×7=42 格,首行起于上月月末、末行止于下月月初(邻月格可点选)。 */
const cells = computed<Array<{ day: number; date: Date; inMonth: boolean; disabled: boolean }>>(() => {
  const first = new Date(viewYear.value, viewMonth.value, 1)
  const startOffset = first.getDay()
  const gridStart = new Date(viewYear.value, viewMonth.value, 1 - startOffset)
  const result = []
  for (let index = 0; index < 42; index++) {
    const date = new Date(
      gridStart.getFullYear(),
      gridStart.getMonth(),
      gridStart.getDate() + index,
    )
    const time = date.getTime()
    const disabled =
      (Number.isFinite(props.min) && time < (props.min ?? 0)) ||
      (Number.isFinite(props.max) && time > (props.max ?? 0))
    result.push({
      day: date.getDate(),
      date,
      inMonth: date.getFullYear() === viewYear.value && date.getMonth() === viewMonth.value,
      disabled,
    })
  }
  return result
})

const isToday = (date: Date) => {
  const now = new Date()
  return (
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate()
  )
}

const isSelected = (date: Date) => {
  if (props.value === null || props.value === undefined) return false
  const selected = new Date(props.value)
  return (
    date.getFullYear() === selected.getFullYear() &&
    date.getMonth() === selected.getMonth() &&
    date.getDate() === selected.getDate()
  )
}

function pick(cell: (typeof cells.value)[number]): void {
  if (cell.disabled) return
  // 点选邻月日期:同步视图到该月再回填
  if (!cell.inMonth) {
    viewYear.value = cell.date.getFullYear()
    viewMonth.value = cell.date.getMonth()
  }
  emit('update:value', cell.date.getTime())
  open.value = false
}

function shiftMonth(delta: number): void {
  const next = new Date(viewYear.value, viewMonth.value + delta, 1)
  viewYear.value = next.getFullYear()
  viewMonth.value = next.getMonth()
}

function shiftYear(delta: number): void {
  const next = new Date(viewYear.value + delta, viewMonth.value, 1)
  viewYear.value = next.getFullYear()
  viewMonth.value = next.getMonth()
}

function onAction(): void {
  emit('action')
}

async function toggle(): Promise<void> {
  if (props.disabled) return
  const next = !open.value
  open.value = next
  if (next) {
    if (props.value) {
      const selected = new Date(props.value)
      viewYear.value = selected.getFullYear()
      viewMonth.value = selected.getMonth()
    }
    await nextTick()
    positionPanel()
  }
}

function positionPanel(): void {
  const root = rootRef.value
  if (!root) return
  const rect = root.getBoundingClientRect()
  panelStyle.value = {
    top: `${rect.bottom + 4}px`,
    left: `${Math.max(4, rect.left)}px`,
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

watch(open, (value) => {
  if (value) {
    window.addEventListener('scroll', onScrollOrResize, true)
    window.addEventListener('resize', onScrollOrResize)
    document.addEventListener('mousedown', onClickOutside)
    window.addEventListener('keydown', onKeydown)
  } else {
    window.removeEventListener('scroll', onScrollOrResize, true)
    window.removeEventListener('resize', onScrollOrResize)
    document.removeEventListener('mousedown', onClickOutside)
    window.removeEventListener('keydown', onKeydown)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
  document.removeEventListener('mousedown', onClickOutside)
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div ref="rootRef" class="ui-datepicker" :class="[`ui-datepicker--${size}`, { 'is-disabled': disabled }]">
    <button
      type="button"
      class="ui-datepicker__trigger"
      :disabled="disabled"
      :aria-expanded="open"
      aria-haspopup="dialog"
      @click="toggle"
    >
      <svg class="ui-datepicker__icon" viewBox="0 0 16 16" aria-hidden="true">
        <rect x="1.5" y="2.5" width="13" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.4" />
        <path d="M1.5 6.5h13M5 1v3M11 1v3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      </svg>
      <span class="ui-datepicker__text" :class="{ 'is-empty': !displayText }">
        {{ displayText || placeholder }}
      </span>
      <span
        v-if="clearable && displayText && !disabled"
        class="ui-datepicker__clear"
        role="button"
        aria-label="清空"
        tabindex="-1"
        @click.stop="emit('update:value', null)"
      >
        <svg viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8" /></svg>
      </span>
    </button>

    <Teleport to="body">
      <Transition name="ui-datepicker-pop">
        <div v-if="open" ref="panelRef" class="ui-datepicker__panel" :style="panelStyle" role="dialog" aria-label="选择日期">
          <header class="ui-datepicker__nav">
            <span class="ui-datepicker__nav-group">
              <button type="button" class="ui-datepicker__nav-btn" aria-label="上一年" @click="shiftYear(-1)">
                &lt;&lt;
              </button>
              <button type="button" class="ui-datepicker__nav-btn" aria-label="上个月" @click="shiftMonth(-1)">
                &lt;
              </button>
            </span>
            <span class="ui-datepicker__nav-title">{{ viewYear }}年{{ viewMonth + 1 }}月</span>
            <span class="ui-datepicker__nav-group">
              <button type="button" class="ui-datepicker__nav-btn" aria-label="下个月" @click="shiftMonth(1)">
                &gt;
              </button>
              <button type="button" class="ui-datepicker__nav-btn" aria-label="下一年" @click="shiftYear(1)">
                &gt;&gt;
              </button>
            </span>
          </header>
          <div class="ui-datepicker__week">
            <span v-for="label in weekLabels" :key="label">{{ label }}</span>
          </div>
          <div class="ui-datepicker__grid">
            <button
              v-for="(cell, index) in cells"
              :key="index"
              type="button"
              class="ui-datepicker__cell"
              :class="{
                'is-adjacent': !cell.inMonth,
                'is-disabled': cell.disabled,
                'is-selected': isSelected(cell.date),
                'is-today': isToday(cell.date),
              }"
              :disabled="cell.disabled"
              @click="pick(cell)"
            >
              {{ cell.day }}
            </button>
          </div>
          <footer v-if="actionText" class="ui-datepicker__footer">
            <button type="button" class="ui-datepicker__action" @click="onAction">{{ actionText }}</button>
          </footer>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.ui-datepicker {
  display: inline-flex;
  box-sizing: border-box;
}

.ui-datepicker__trigger {
  display: inline-flex;
  width: 100%;
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
}

.ui-datepicker--small .ui-datepicker__trigger {
  height: 28px;
  font-size: 14px;
}

.ui-datepicker--medium .ui-datepicker__trigger {
  height: 34px;
  font-size: var(--ui-font-base, 14px);
}

.ui-datepicker.is-disabled .ui-datepicker__trigger {
  cursor: not-allowed;
  opacity: 0.75;
  background: var(--ui-bg-header, #fafafa);
}

.ui-datepicker__trigger:hover:not(:disabled),
.ui-datepicker__trigger[aria-expanded='true'] {
  border-color: var(--ui-color-primary, #409eff);
}

.ui-datepicker__icon {
  width: 15px;
  height: 15px;
  flex: 0 0 auto;
  color: var(--ui-text-secondary, #909399);
}

.ui-datepicker__text {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ui-datepicker__text.is-empty {
  color: var(--ui-text-placeholder, #c0c4cc);
}

.ui-datepicker__clear {
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

.ui-datepicker__clear svg {
  width: 9px;
  height: 9px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
}

/* 弹层 */
.ui-datepicker__panel {
  position: fixed;
  z-index: 12000;
  width: 264px;
  box-sizing: border-box;
  padding: 10px;
  border: 1px solid var(--ui-border-light, #e4e7ed);
  border-radius: var(--ui-radius-lg, 8px);
  background: #fff;
  box-shadow: var(--ui-shadow-dropdown, 0 4px 16px rgb(0 0 0 / 12%));
}

.ui-datepicker__nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
  gap: 4px;
}

.ui-datepicker__nav-group {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.ui-datepicker__nav-title {
  min-width: 0;
  flex: 1;
  font-size: var(--ui-font-base, 14px);
  font-weight: 600;
  color: var(--ui-text-main, #303133);
  text-align: center;
  white-space: nowrap;
}

.ui-datepicker__nav-btn {
  display: inline-flex;
  min-width: 24px;
  height: 24px;
  box-sizing: border-box;
  align-items: center;
  justify-content: center;
  padding: 0 5px;
  border: 0;
  border-radius: var(--ui-radius-sm, 4px);
  color: var(--ui-text-regular, #606266);
  background: transparent;
  cursor: pointer;
  font-size: var(--ui-font-xs, 12px);
  font-weight: 700;
  font-family: Consolas, 'Courier New', monospace;
  line-height: 1;
}

.ui-datepicker__nav-btn:hover {
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary-weak, #ecf5ff);
}

.ui-datepicker__nav-btn svg {
  width: 8px;
  height: 12px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.ui-datepicker__week,
.ui-datepicker__grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
}

.ui-datepicker__week span {
  padding: 4px 0;
  color: var(--ui-text-secondary, #909399);
  font-size: var(--ui-font-xs, 12px);
  text-align: center;
}

.ui-datepicker__cell {
  display: grid;
  height: 28px;
  place-items: center;
  border: 0;
  border-radius: var(--ui-radius-sm, 4px);
  color: var(--ui-text-main, #303133);
  background: transparent;
  cursor: pointer;
  font-size: var(--ui-font-xs, 12px);
}

.ui-datepicker__cell:hover:not(:disabled) {
  background: var(--ui-bg-hover, #f5f7fa);
}

.ui-datepicker__cell.is-adjacent {
  color: var(--ui-text-secondary, #909399);
  font-weight: 400;
}

.ui-datepicker__cell.is-adjacent:hover:not(:disabled) {
  color: var(--ui-text-regular, #606266);
}

.ui-datepicker__cell.is-adjacent.is-selected {
  color: #fff;
}

.ui-datepicker__cell.is-disabled {
  cursor: not-allowed;
  color: var(--ui-text-disabled, #a8abb2);
  opacity: 0.5;
}

.ui-datepicker__cell.is-today {
  color: var(--ui-color-primary, #409eff);
  font-weight: 700;
}

.ui-datepicker__cell.is-selected {
  color: #fff;
  background: var(--ui-color-primary, #409eff);
}

.ui-datepicker__cell.is-selected.is-today {
  color: #fff;
}

.ui-datepicker__footer {
  display: flex;
  justify-content: center;
  padding-top: 6px;
  margin-top: 6px;
  border-top: 1px solid var(--ui-border-extra-light, #f2f3f5);
}

.ui-datepicker__action {
  height: 24px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--ui-radius-sm, 4px);
  color: var(--ui-color-primary, #409eff);
  background: transparent;
  cursor: pointer;
  font-size: var(--ui-font-xs, 12px);
}

.ui-datepicker__action:hover {
  background: var(--ui-color-primary-weak, #ecf5ff);
}

.ui-datepicker-pop-enter-active,
.ui-datepicker-pop-leave-active {
  transition:
    opacity 140ms ease,
    transform 140ms ease;
}

.ui-datepicker-pop-enter-from,
.ui-datepicker-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
