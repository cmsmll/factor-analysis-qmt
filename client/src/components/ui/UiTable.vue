<script setup lang="ts">
import {
  computed,
  defineComponent,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type VNode,
} from 'vue'

defineOptions({ name: 'UiTable' })

/** 承载列 render 返回的 VNode/字符串。 */
const RenderNode = defineComponent({
  name: 'UiTableRenderNode',
  props: ['node'] as const,
  render() {
    const node: unknown = this.node
    return node === null || node === undefined ? null : (node as VNode)
  },
})

/** 与 naive DataTableColumns 用法兼容的列定义（覆盖本项目用到的面）。 */
export interface UiTableColumn<T = any> {
  title?: string
  key?: string
  /** 数值按 px 参与固定列宽；缺省列由组件按剩余宽度均分（自适应） */
  width?: number | string
  align?: 'left' | 'center' | 'right'
  /** 显式关闭该列排序（默认除操作列外全部可排） */
  sortable?: boolean
  /** 兼容旧写法：true 表示可排/受控标识；函数提供自定义比较（缺省按行值数值或中文本地化比较） */
  sorter?: boolean | ((a: T, b: T) => number)
  /** 受控排序态（naive 兼容，外部绑定后由父级驱动并接收 update:sorter） */
  sortOrder?: 'ascend' | 'descend' | false
  render?: (row: T, index: number) => unknown
  type?: 'expand' | 'selection'
  /** type=expand 时：展开区内容 */
  renderExpand?: (row: T) => unknown
  className?: string
}

type SortOrder = 'ascend' | 'descend' | false

const props = withDefaults(
  defineProps<{
    columns: UiTableColumn[]
    data: unknown[]
    rowKey?: (row: any, index: number) => string | number
    rowProps?: (row: any) => { style?: Record<string, string>; onClick?: () => void }
    size?: 'small' | 'medium' | 'large'
    bordered?: boolean
    singleLine?: boolean
    loading?: boolean
    scrollX?: number | string
    /** naive 兼容：v-model:expanded-row-keys */
    expandedRowKeys?: Array<string | number>
    /** naive 兼容：v-model:checked-row-keys */
    checkedRowKeys?: Array<string | number>
    emptyText?: string
  }>(),
  { size: 'medium', bordered: false, singleLine: true, loading: false, emptyText: '暂无数据' },
)

const emit = defineEmits<{
  (e: 'update:expandedRowKeys', keys: Array<string | number>): void
  (e: 'update:checkedRowKeys', keys: Array<string | number>): void
  (e: 'update:sorter', sorter: { columnKey: string; order: SortOrder } | null): void
}>()

type RowKey = string | number

function keyOf(row: unknown, index: number): RowKey {
  return props.rowKey ? props.rowKey(row, index) : index
}

// ── 列宽：显式 px 优先，其余均分剩余宽度(自适应) ──
const rootRef = ref<HTMLDivElement | null>(null)
const colWidths = ref<number[]>([])
const isHScroll = ref(false)

function refreshWidths(): void {
  const root = rootRef.value
  if (!root) return
  const border = props.bordered ? 2 : 0
  const available = Math.max(root.clientWidth - border, 0)
  const fixed = props.columns.reduce(
    (sum, column) => sum + (typeof column.width === 'number' ? column.width : 0),
    0,
  )
  const autoCount = props.columns.filter((column) => typeof column.width !== 'number').length
  const auto = autoCount > 0 ? Math.max(64, Math.floor((available - fixed) / autoCount)) : 0
  colWidths.value = props.columns.map((column) =>
    typeof column.width === 'number' ? column.width : auto,
  )
  const total = colWidths.value.reduce((sum, width) => sum + width, 0)
  // 仅在列宽总和确实超过容器时才启用横向滚动(否则容器会破坏表头吸顶)
  isHScroll.value = total > available + 2
}

let widthObserver: ResizeObserver | null = null

watch(
  () => props.columns.map((column) => `${column.key ?? ''}:${column.width ?? ''}`).join('|'),
  () => refreshWidths(),
)

onMounted(() => {
  refreshWidths()
  if (rootRef.value && typeof ResizeObserver !== 'undefined') {
    widthObserver = new ResizeObserver(() => refreshWidths())
    widthObserver.observe(rootRef.value)
  }
})

onBeforeUnmount(() => {
  widthObserver?.disconnect()
  widthObserver = null
})

const totalMinWidth = computed(() => colWidths.value.reduce((sum, width) => sum + width, 0))

// ── 排序：除操作列/序号外全部数据列默认可排 ──
type ColumnSortState = { key: string; order: SortOrder; fn?: (a: unknown, b: unknown) => number }

const internalSorter = ref<ColumnSortState | null>(null)

/** 该列是否参与排序（操作列与序号列默认排除，显式 sortable:false 排除）。 */
function columnSortable(column: UiTableColumn): boolean {
  if (column.type === 'expand' || column.type === 'selection') return false
  if (column.sortable === false) return false
  const key = String(column.key ?? '')
  const title = String(column.title ?? '')
  if (key === 'no' || key === 'rank' || title === '序号' || title === '#') return false
  return true
}

function resolveSortValue(row: unknown, key: string): string | number | Array<unknown> | null {
  const record = row as Record<string, unknown>
  const value = record[key]
  if (value === null || value === undefined) return null
  return value as string | number | Array<unknown>
}

function compareRows(
  left: unknown,
  right: unknown,
  key: string,
  fn?: (a: unknown, b: unknown) => number,
): number {
  if (fn) return fn(left, right)
  const lv = resolveSortValue(left, key)
  const rv = resolveSortValue(right, key)
  if (typeof lv === 'number' && typeof rv === 'number') return lv - rv
  const lText = Array.isArray(lv) ? lv.join(' ') : String(lv ?? '')
  const rText = Array.isArray(rv) ? rv.join(' ') : String(rv ?? '')
  return lText.localeCompare(rText, 'zh-CN', { numeric: true, sensitivity: 'base' })
}

const sortedData = computed(() => {
  const state = internalSorter.value
  if (!state || state.order === false) return props.data
  const column = props.columns.find((item) => item.key === state.key)
  const fn = typeof column?.sorter === 'function' ? (column.sorter as (a: unknown, b: unknown) => number) : undefined
  const direction = state.order === 'ascend' ? 1 : -1
  const copy = [...props.data]
  copy.sort((a, b) => compareRows(a, b, state.key, fn) * direction)
  return copy
})

function cycleOrder(current: SortOrder): SortOrder {
  return current === 'ascend' ? 'descend' : current === 'descend' ? false : 'ascend'
}

function handleHeaderClick(column: UiTableColumn): void {
  if (!columnSortable(column)) return
  const key = column.key
  if (!key) return
  let current: SortOrder
  if (column.sortOrder !== undefined) {
    current = column.sortOrder
  } else {
    const state = internalSorter.value
    current = state && state.key === key ? state.order : false
  }
  const next = cycleOrder(current)

  // 受控列(页面传入 sortOrder):只发事件由父级排;否则组件内部排序
  if (column.sortOrder === undefined) {
    if (next === false) {
      internalSorter.value = null
    } else {
      internalSorter.value = { key, order: next }
    }
  }
  emit('update:sorter', next === false ? null : { columnKey: key, order: next })
}

function orderOf(column: UiTableColumn): SortOrder {
  if (column.sortOrder !== undefined) return column.sortOrder
  const state = internalSorter.value
  return state && state.key === column.key ? state.order : false
}

// ── 展开行 ──
const localExpanded = ref<RowKey[]>(props.expandedRowKeys ?? [])
watch(
  () => props.expandedRowKeys,
  (keys) => {
    localExpanded.value = [...(keys ?? [])]
  },
)

function toggleExpand(row: unknown, index: number, event?: MouseEvent): void {
  event?.stopPropagation()
  event?.preventDefault()
  const key = keyOf(row, index)
  const next = localExpanded.value.includes(key)
    ? localExpanded.value.filter((item) => item !== key)
    : [...localExpanded.value, key]
  localExpanded.value = next
  emit('update:expandedRowKeys', next)
}

function handleRowClick(row: unknown): void {
  if (!props.rowProps) return
  props.rowProps(row)?.onClick?.()
}

// ── 勾选列 ──
const localChecked = ref<RowKey[]>(props.checkedRowKeys ?? [])
watch(
  () => props.checkedRowKeys,
  (keys) => {
    localChecked.value = [...(keys ?? [])]
  },
)

function isChecked(row: unknown, index: number): boolean {
  return localChecked.value.includes(keyOf(row, index))
}

function toggleCheck(row: unknown, index: number): void {
  const key = keyOf(row, index)
  const next = isChecked(row, index)
    ? localChecked.value.filter((item) => item !== key)
    : [...localChecked.value, key]
  localChecked.value = next
  emit('update:checkedRowKeys', next)
}

function toggleCheckAll(): void {
  const keys = props.data.map((row, index) => keyOf(row, index))
  const allChecked = props.data.length > 0 && keys.every((key) => localChecked.value.includes(key))
  localChecked.value = allChecked ? [] : keys
  emit('update:checkedRowKeys', localChecked.value)
}

const allChecked = computed(
  () =>
    props.data.length > 0 &&
    props.data.every((row, index) => localChecked.value.includes(keyOf(row, index))),
)

/** 取单元格内容（字符串或 VNode，经 RenderNode 渲染）。 */
function cellContent(column: UiTableColumn, row: unknown, rowIndex: number): unknown {
  if (column.render) return column.render(row, rowIndex)
  return (row as Record<string, unknown>)[String(column.key ?? '')]
}

/** 取展开列 renderExpand 输出。 */
function expandContent(row: unknown): unknown {
  const column = props.columns.find((item) => item.type === 'expand')
  return column?.renderExpand ? column.renderExpand(row) : null
}
</script>

<template>
  <div
    ref="rootRef"
    class="ui-table"
    :class="[
      `ui-table--${size}`,
      { 'has-border': bordered, 'is-single-line': singleLine },
    ]"
  >
    <div class="ui-table__scroll" :class="{ 'is-hscroll': isHScroll }">
      <table
        class="ui-table__table"
        :style="{
          width: isHScroll ? `${Math.max(totalMinWidth, 100)}px` : '100%',
          minWidth: scrollX !== undefined ? (typeof scrollX === 'number' ? `${scrollX}px` : scrollX) : undefined,
        }"
      >
        <colgroup>
          <col v-for="(column, index) in columns" :key="index" :style="{ width: `${colWidths[index] ?? 100}px` }" />
        </colgroup>
        <thead class="ui-table__head">
          <tr>
            <th
              v-for="(column, index) in columns"
              :key="index"
              class="ui-table__th"
              :class="{ 'is-sortable': columnSortable(column) }"
              scope="col"
              :aria-sort="orderOf(column) === 'ascend' ? 'ascending' : orderOf(column) === 'descend' ? 'descending' : undefined"
              @click="handleHeaderClick(column)"
              @keydown.enter.prevent="handleHeaderClick(column)"
            >
              <template v-if="column.type === 'selection'">
                <label class="ui-table__check" @click.stop>
                  <input type="checkbox" :checked="allChecked" @change="toggleCheckAll" />
                  <span class="ui-table__checkmark"></span>
                </label>
              </template>
              <template v-else-if="column.type === 'expand'"></template>
              <span v-else-if="columnSortable(column)" class="ui-table__sort-grid">
                <span class="ui-table__side" aria-hidden="true"></span>
                <span class="ui-table__title">{{ column.title ?? '' }}</span>
                <span
                  class="ui-table__side ui-table__sort"
                  :class="orderOf(column) !== false ? 'is-active' : ''"
                  aria-hidden="true"
                >
                  <svg v-if="orderOf(column) === 'ascend'" class="ui-table__sort-arrow" viewBox="0 0 1024 1024">
                    <path
                      d="M877.863693 338.744408 557.862219 18.745191c-24.991331-24.993589-65.516166-24.993589-90.509755 0L147.353249 338.744408c-24.989073 24.993589-24.989073 65.516166 0 90.509755 24.993589 24.995847 65.518424 24.995847 90.509755 0l210.745399-210.747656 0 741.49227c0 35.347753 28.653444 64.001198 64.001198 64.001198 35.343237 0 63.99894-28.651187 63.99894-64.001198l0.002257-741.49227 210.747656 210.745399c12.494537 12.496794 28.874707 18.74632 45.25262 18.74632s32.758083-6.247268 45.254877-18.744063C902.855024 404.258316 902.855024 363.740254 877.863693 338.744408z"
                    />
                  </svg>
                  <svg v-else-if="orderOf(column) === 'descend'" class="ui-table__sort-arrow is-down" viewBox="0 0 1024 1024">
                    <path
                      d="M877.863693 338.744408 557.862219 18.745191c-24.991331-24.993589-65.516166-24.993589-90.509755 0L147.353249 338.744408c-24.989073 24.993589-24.989073 65.516166 0 90.509755 24.993589 24.995847 65.518424 24.995847 90.509755 0l210.745399-210.747656 0 741.49227c0 35.347753 28.653444 64.001198 64.001198 64.001198 35.343237 0 63.99894-28.651187 63.99894-64.001198l0.002257-741.49227 210.747656 210.745399c12.494537 12.496794 28.874707 18.74632 45.25262 18.74632s32.758083-6.247268 45.254877-18.744063C902.855024 404.258316 902.855024 363.740254 877.863693 338.744408z"
                    />
                  </svg>
                </span>
              </span>
              <span v-else class="ui-table__title">{{ column.title ?? '' }}</span>
            </th>
          </tr>
        </thead>
        <tbody class="ui-table__body">
          <template v-for="(row, rowIndex) in sortedData" :key="keyOf(row, rowIndex)">
            <tr
              class="ui-table__row"
              :style="rowProps ? rowProps(row).style : undefined"
              @click="handleRowClick(row)"
            >
              <td v-for="(column, columnIndex) in columns" :key="columnIndex" class="ui-table__td">
                <template v-if="column.type === 'selection'">
                  <label class="ui-table__check">
                    <input
                      type="checkbox"
                      :checked="isChecked(row, rowIndex)"
                      @change="toggleCheck(row, rowIndex)"
                    />
                    <span class="ui-table__checkmark"></span>
                  </label>
                </template>
                <button
                  v-else-if="column.type === 'expand'"
                  type="button"
                  class="ui-table__expand"
                  :class="{ 'is-open': localExpanded.includes(keyOf(row, rowIndex)) }"
                  aria-label="展开/收起"
                  @click="toggleExpand(row, rowIndex, $event)"
                >
                  <svg viewBox="0 0 8 12"><path d="M1.5 1l5 5-5 5" /></svg>
                </button>
                <RenderNode v-else :node="cellContent(column, row, rowIndex)" />
              </td>
            </tr>
            <tr v-if="localExpanded.includes(keyOf(row, rowIndex))" class="ui-table__expand-row">
              <td class="ui-table__expand-cell" :colspan="columns.length">
                <RenderNode :node="expandContent(row)" />
              </td>
            </tr>
          </template>
          <tr v-if="loading" class="ui-table__empty-row">
            <td class="ui-table__empty-cell" :colspan="columns.length">
              <div class="ui-table__loading">
                <span class="ui-table__loading-spin"></span>
                <span>加载中...</span>
              </div>
            </td>
          </tr>
          <tr v-else-if="!loading && sortedData.length === 0" class="ui-table__empty-row">
            <td class="ui-table__empty-cell" :colspan="columns.length">{{ emptyText }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.ui-table {
  width: 100%;
  box-sizing: border-box;
  background: var(--ui-bg-card, #fff);
  border-radius: var(--ui-radius-lg, 8px);
  /* 容器不裁剪内容：吸顶表头需要 overflow 可见才能相对视口滚动 */
}

.ui-table.has-border {
  border: 1px solid var(--ui-border-lighter, #ebeef5);
  box-shadow: var(--ui-shadow-card, 0 1px 3px rgb(0 0 0 / 6%));
}

/* 列宽超容器时横向滚动(仅此情形启用,避免容器成为滚动容器破坏表头吸顶) */
.ui-table__scroll {
  width: 100%;
  border-radius: var(--ui-radius-lg, 8px);
}

.ui-table__scroll.is-hscroll {
  overflow-x: auto;
}

.ui-table__table {
  border-spacing: 0;
  border-collapse: separate;
  table-layout: fixed;
  color: var(--ui-text-control, #333639);
  font-size: var(--ui-font-sm, 13px);
}

.ui-table--small .ui-table__table {
  font-size: var(--ui-font-xs, 12px);
}

/* 表头与内容统一居中 */
.ui-table__th,
.ui-table__td {
  box-sizing: border-box;
  padding: 8px 6px;
  text-align: center;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: middle;
}

.ui-table--small .ui-table__th,
.ui-table--small .ui-table__td {
  padding: 6px 6px;
}

/* 圆角：容器 overflow 保持可见(吸顶)，角部由表头/末行单元格自身裁切 */
.ui-table__th:first-child {
  border-top-left-radius: calc(var(--ui-radius-lg, 8px) - 1px);
}

.ui-table__th:last-child {
  border-top-right-radius: calc(var(--ui-radius-lg, 8px) - 1px);
}

.ui-table__body tr:last-child .ui-table__td:first-child {
  border-bottom-left-radius: calc(var(--ui-radius-lg, 8px) - 1px);
}

.ui-table__body tr:last-child .ui-table__td:last-child {
  border-bottom-right-radius: calc(var(--ui-radius-lg, 8px) - 1px);
}

/* 表头吸顶 */
.ui-table__th {
  position: sticky;
  top: 0;
  z-index: 5;
  color: var(--ui-text-regular, #606266);
  background: var(--ui-bg-header, #fafafa);
  font-weight: 600;
}

/* 标题居中：可排序列用「左右等宽预留 | 标题 | 排序钮」栅格;不可排序列(含序号)直接居中 */
.ui-table__title {
  display: inline-block;
  max-width: 100%;
  line-height: 1.4;
  vertical-align: middle;
  /* 表头标题允许多行(如 mode1 双行标题);单元格正文仍单行省略 */
  white-space: pre-line;
  word-break: break-all;
}

.ui-table__sort-grid {
  display: grid;
  width: 100%;
  grid-template-columns: 18px minmax(0, 1fr) 18px;
  align-items: center;
}

.ui-table__sort-grid .ui-table__title {
  grid-column: 2;
  justify-self: center;
}

.ui-table__th.is-sortable {
  cursor: pointer;
  user-select: none;
}

.ui-table__th.is-sortable:hover {
  color: var(--ui-text-main, #303133);
}

.ui-table__side {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

/* 排序箭头：默认不显示，仅当前排序列显示 ↑ / ↓ */
.ui-table__sort {
  grid-column: 3;
  justify-self: center;
  color: var(--ui-border-light, #e4e7ed);
}

.ui-table__sort-arrow {
  width: 14px;
  height: 16px;
  display: block;
  fill: currentColor;
}

/* 下行箭头:同一长箭头路径垂直翻转 */
.ui-table__sort-arrow.is-down {
  transform: scaleY(-1);
}

.ui-table__sort.is-active {
  color: var(--ui-color-primary, #409eff);
}

.ui-table__td {
  border-bottom: 1px solid var(--ui-border-extra-light, #f2f3f5);
}

.ui-table__body tr:last-child .ui-table__td {
  border-bottom: 0;
}

.ui-table__body .ui-table__row:hover .ui-table__td {
  background: var(--ui-bg-hover, #f5f7fa);
}

.ui-table.is-single-line .ui-table__row {
  height: 40px;
}

.ui-table--small.is-single-line .ui-table__row {
  height: 34px;
}

/* 勾选 */
.ui-table__check {
  display: inline-flex;
  align-items: center;
  cursor: pointer;
}

.ui-table__check input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}

.ui-table__checkmark {
  position: relative;
  width: 15px;
  height: 15px;
  box-sizing: border-box;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: 3px;
  background: #fff;
}

.ui-table__checkmark::after {
  position: absolute;
  top: 2px;
  left: 4px;
  width: 4px;
  height: 8px;
  border: solid #fff;
  border-width: 0 2px 2px 0;
  content: '';
  opacity: 0;
  transform: rotate(45deg);
}

.ui-table__check input:checked + .ui-table__checkmark {
  border-color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary, #409eff);
}

.ui-table__check input:checked + .ui-table__checkmark::after {
  opacity: 1;
}

/* 展开行 */
.ui-table__expand {
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 4px;
  color: var(--ui-text-secondary, #909399);
  background: transparent;
  cursor: pointer;
  padding: 0;
}

.ui-table__expand:hover {
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-bg-hover, #f5f7fa);
}

.ui-table__expand svg {
  width: 8px;
  height: 12px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
  transition: transform 160ms ease;
}

.ui-table__expand.is-open svg {
  transform: rotate(90deg);
}

.ui-table__expand-row .ui-table__expand-cell {
  padding: 0;
  background: var(--ui-bg-header, #fafafa);
}

/* 空/加载态 */
.ui-table__empty-row .ui-table__empty-cell {
  height: 90px;
  color: var(--ui-text-secondary, #909399);
  text-align: center;
}

.ui-table__loading {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.ui-table__loading-spin {
  width: 14px;
  height: 14px;
  border: 2px solid var(--ui-border-light, #e4e7ed);
  border-top-color: var(--ui-color-primary, #409eff);
  border-radius: 50%;
  animation: ui-table-spin 0.8s linear infinite;
}

@keyframes ui-table-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
