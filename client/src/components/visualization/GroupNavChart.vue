<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { NButton, NRadioGroup, NRadio } from 'naive-ui'
import { fetchIndiceHistory, fetchIndices } from '@/api/mode1'
import CloseIcon from '@/assets/icons/icon-close.svg'
import SearchIcon from '@/assets/icons/search.svg'
import { sumReturnPercentSeries, formatDate } from '@/utils/factorSeries'
import { useChartKeyboardPointer } from '@/utils/chartKeyboard'
import { useGlobalMessageStore } from '@/stores/globalMessage'
import type { IndicePoint } from '@/types/mode1'

const props = withDefaults(
  defineProps<{
    datetimes?: string[]
    changePercent?: number[][]
    quantileNames?: string[]
    quantileCount?: number
    loading?: boolean
  }>(),
  {
    datetimes: () => [],
    changePercent: () => [],
    quantileNames: () => [],
    quantileCount: 5,
    loading: false,
  },
)

const emit = defineEmits<{
  'update:quantileCount': [value: number]
  'select-date': [date: string]
}>()

/** 点击图表 x 轴某日时回调对应日期（YYYY-MM-DD），供父级跳转单日明细。 */
type ChartInstance = {
  convertFromPixel: (payload: { seriesIndex: number }, point: number[]) => Array<number | string> | undefined
  getDom: () => HTMLElement
}

/** 键盘指针用实例与点击反解用实例共用同一图表实例。 */
const chartInstance = ref<ChartInstance | null>(null)
function onChartRef(instance: unknown): void {
  setChartRef(instance)
  chartInstance.value = isChartInstance(instance) ? instance : null
}

function isChartInstance(value: unknown): value is ChartInstance {
  return (
    typeof value === 'object' &&
    value !== null &&
    'convertFromPixel' in value &&
    'getDom' in value
  )
}

function onChartClick(event: MouseEvent): void {
  const chart = chartInstance.value
  if (!chart) return
  const rect = chart.getDom().getBoundingClientRect()
  const point = chart.convertFromPixel(
    { seriesIndex: 0 },
    [event.clientX - rect.left, event.clientY - rect.top],
  )
  const value = point?.[0]
  if (typeof value === 'number' && axisDates.value[value]) {
    emit('select-date', axisDates.value[value])
  } else if (typeof value === 'string' && value) {
    emit('select-date', value)
  }
}
const globalMessage = useGlobalMessageStore()
const sortMode = ref(0)
const showPicker = ref(false)
const indiceList = ref<string[]>([])
const indiceLoading = ref(false)
const indiceError = ref('')
const keyword = ref('')

/** 搜索词过滤后的指数列表。 */
const filteredIndices = computed(() => {
  const value = keyword.value.trim().toLocaleLowerCase()
  if (!value) return indiceList.value
  return indiceList.value.filter((item) => item.toLocaleLowerCase().includes(value))
})

watch(showPicker, (value) => {
  if (value) keyword.value = ''
})

/** 已勾选指数及其历史收益数据。 */
const checkedIndices = ref<Map<string, IndicePoint[]>>(new Map())
/** 各指数历史请求进行中标记。 */
const pendingIndices = ref<Set<string>>(new Set())

const sortLabel = computed(() => {
  if (sortMode.value === 1) return '↓ 降序'
  if (sortMode.value === 2) return '↑ 升序'
  return '默认'
})

const COLORS = [
  '#91cc75',
  '#73c0de',
  '#5470c6',
  '#ee6666',
  '#fac858',
  '#e74c3c',
  '#2ecc71',
  '#3498db',
  '#9b59b6',
  '#34495e',
]

const selectedSeries = computed(() => props.changePercent)

const selectedNames = computed(() => props.quantileNames)
const { setChartRef, handleChartFocus, handleChartKeydown, handleChartMousemove } =
  useChartKeyboardPointer(computed(() => props.datetimes.length))

/** 图表 x 轴日期（YYYY-MM-DD）。 */
const axisDates = computed(() => props.datetimes.map(formatDate))

/**
 * 指数历史收益对齐到图表 x 轴：
 * - 只渲染 x 轴上存在的日期（交集）；
 * - 指数数据比图表范围短时，左侧或右侧缺失日期用 0 收益补全。
 */
function alignIndiceSeries(points: IndicePoint[]): number[] {
  const byDate = new Map(points.map((point) => [point.datetime, point.profit]))
  return axisDates.value.map((date) => byDate.get(date) ?? 0)
}

const indiceSeries = computed(() =>
  [...checkedIndices.value.entries()].map(([name, points]) => ({
    name,
    type: 'line' as const,
    data: sumReturnPercentSeries(alignIndiceSeries(points)),
    smooth: true,
    showSymbol: false,
    lineStyle: { width: 1.5, type: 'dotted' as const },
  })),
)

const option = computed(() => ({
  tooltip: {
    trigger: 'axis' as const,
    axisPointer: { type: 'cross' as const, snap: true },
    valueFormatter: (value: number) => `${value.toFixed(2)}%`,
    ...(sortMode.value !== 0
      ? { order: sortMode.value === 1 ? ('valueDesc' as const) : ('valueAsc' as const) }
      : {}),
  },
  legend: {
    bottom: 0,
    type: 'scroll' as const,
    data: [...selectedNames.value, ...checkedIndices.value.keys()],
  },
  grid: { top: 20, left: 60, right: 30, bottom: 70 },
  xAxis: {
    type: 'category' as const,
    data: axisDates.value,
  },
  yAxis: {
    type: 'value' as const,
    axisLabel: { formatter: '{value}%' },
  },
  dataZoom: [
    { type: 'inside' as const, start: 0, end: 100 },
    {
      type: 'slider' as const,
      height: 8,
      bottom: 30,
      disabled: true,
      showDetail: false,
      showDataShadow: false,
      brushSelect: false,
      handleSize: 0,
      moveHandleSize: 0,
      borderColor: 'transparent',
      backgroundColor: '#edf1f7',
      fillerColor: '#8aa4d6',
      handleStyle: { opacity: 0 },
      moveHandleStyle: { opacity: 0 },
      emphasis: {
        handleStyle: { opacity: 0 },
        moveHandleStyle: { opacity: 0 },
      },
    },
  ],
  color: COLORS,
  series: [
    ...selectedSeries.value.map((series, index) => ({
      name: selectedNames.value[index] ?? `切片${index + 1}`,
      type: 'line' as const,
      data: sumReturnPercentSeries(series),
      smooth: true,
      showSymbol: false,
      lineStyle: { width: index === 0 || index === selectedSeries.value.length - 1 ? 2 : 1.5 },
    })),
    ...indiceSeries.value,
  ],
}))

async function openPicker(): Promise<void> {
  showPicker.value = true
  if (indiceList.value.length > 0) return

  indiceLoading.value = true
  indiceError.value = ''
  try {
    indiceList.value = await fetchIndices()
  } catch (error) {
    const message = error instanceof Error ? error.message : '指数列表加载失败'
    indiceError.value = message
    globalMessage.error(message)
  } finally {
    indiceLoading.value = false
  }
}

async function toggleIndice(name: string, checked: boolean): Promise<void> {
  if (!checked) {
    checkedIndices.value.delete(name)
    checkedIndices.value = new Map(checkedIndices.value)
    return
  }

  if (checkedIndices.value.has(name)) return
  pendingIndices.value.add(name)
  try {
    const points = await fetchIndiceHistory(name)
    checkedIndices.value.set(name, points)
    checkedIndices.value = new Map(checkedIndices.value)
  } catch (error) {
    // 请求失败：全局错误提示并回滚勾选，不残留错误曲线
    const message = error instanceof Error ? error.message : `${name}历史数据加载失败`
    globalMessage.error(message)
    checkedIndices.value.delete(name)
    checkedIndices.value = new Map(checkedIndices.value)
  } finally {
    pendingIndices.value.delete(name)
  }
}

/** 勾选全部指数（逐个异步加载，复用 toggleIndice 的请求管理）。 */
function selectAllIndices(): void {
  for (const name of filteredIndices.value) {
    if (!checkedIndices.value.has(name)) void toggleIndice(name, true)
  }
}

/** 清空全部已勾选指数。 */
function clearIndices(): void {
  checkedIndices.value.clear()
  checkedIndices.value = new Map(checkedIndices.value)
}

onBeforeUnmount(() => {
  checkedIndices.value.clear()
  pendingIndices.value.clear()
})
</script>

<template>
  <div class="chart-card">
    <div class="chart-card-header">
      <span class="chart-title">分组收益曲线</span>
      <div class="header-controls">
        <NButton size="small" secondary @click="openPicker">指数同框</NButton>
        <NButton class="sort-btn" size="tiny" quaternary @click="sortMode = (sortMode + 1) % 3">
          {{ sortLabel }}
        </NButton>
        <NRadioGroup
          :value="quantileCount"
          :disabled="loading"
          size="small"
          @update:value="emit('update:quantileCount', Number($event))"
        >
          <NRadio :value="3">三分位</NRadio>
          <NRadio :value="5">五分位</NRadio>
          <NRadio :value="10">十分位</NRadio>
        </NRadioGroup>
      </div>
    </div>
        <div class="chart-body" @click="onChartClick">
      <VChart
        :ref="onChartRef"
        :option="option"
        autoresize
        tabindex="0"
        @focus="handleChartFocus"
        @keydown="handleChartKeydown"
        @mousemove="handleChartMousemove"
      />
    </div>
    <Teleport to="body">
      <Transition name="filter-selector">
        <div v-if="showPicker" class="selector-mask" @click.self="showPicker = false">
          <section class="selector-dialog" role="dialog" aria-modal="true" aria-label="指数同框">
            <header class="selector-header">
              <label class="search-box">
                <img :src="SearchIcon" class="search-icon" alt="" />
                <input v-model="keyword" type="search" placeholder="搜索指数" autofocus />
              </label>
              <button class="icon-button" type="button" aria-label="关闭" @click="showPicker = false">
                <img :src="CloseIcon" alt="" />
              </button>
            </header>

            <div class="selector-summary">
              <strong>指数同框</strong>
              <span>已选择 {{ checkedIndices.size }} / {{ indiceList.length }}</span>
            </div>

            <div class="selector-content">
              <div v-if="indiceLoading" class="empty-state">加载中…</div>
              <div v-else-if="indiceError" class="empty-state indice-error">{{ indiceError }}</div>
              <div v-else-if="indiceList.length === 0" class="empty-state">暂无指数数据</div>
              <template v-else>
                <button
                  v-for="name in filteredIndices"
                  :key="name"
                  class="option-item"
                  :class="{ selected: checkedIndices.has(name) }"
                  type="button"
                  @click="toggleIndice(name, !checkedIndices.has(name))"
                >
                  <span class="option-check" aria-hidden="true">{{
                    pendingIndices.has(name) ? '…' : checkedIndices.has(name) ? '✓' : ''
                  }}</span>
                  <span>{{ name }}</span>
                </button>
                <div v-if="filteredIndices.length === 0" class="empty-state">没有匹配的选项</div>
              </template>
            </div>

            <footer class="selector-footer">
              <button type="button" class="footer-button select-all-button" @click="selectAllIndices">
                全选
              </button>
              <button type="button" class="footer-button confirm-button" @click="showPicker = false">
                完成
              </button>
              <button type="button" class="footer-button clear-button" @click="clearIndices">
                清空
              </button>
            </footer>
          </section>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.chart-card {
  background: #fff;
  border-radius: 8px;
  padding: 16px 20px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.chart-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.header-controls {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sort-btn {
  font-size: 14px !important;
  color: rgb(51, 54, 57) !important;
}

.chart-title {
  font-size: 14px;
  font-weight: 600;
  color: #333;
}

.chart-body {
  width: 100%;
  height: 450px;
}

.selector-mask {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(15 23 42 / 0.42);
  backdrop-filter: blur(2px);
}

.selector-dialog {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) auto;
  width: min(720px, 100%);
  height: min(620px, calc(100vh - 48px));
  overflow: hidden;
  border-radius: 12px;
  background: #fff;
  box-shadow: 0 24px 70px rgb(15 23 42 / 0.24);
}

.selector-header {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 18px 20px;
  border-bottom: 1px solid #ebeef5;
}

.search-box {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 10px;
  height: 38px;
  padding: 0 12px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  color: #909399;
  transition: border-color 160ms ease;
}

.search-box:focus-within {
  border-color: #409eff;
}

.search-box input {
  width: 100%;
  border: 0;
  outline: 0;
  color: #303133;
  background: transparent;
  font: inherit;
}

.search-icon {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
}

.icon-button {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  border: 0;
  border-radius: 6px;
  color: #909399;
  background: transparent;
  cursor: pointer;
}

.icon-button:hover {
  color: #409eff;
  background: #ecf5ff;
}

.icon-button svg {
  width: 14px;
  height: 14px;
}

.selector-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px 8px;
  color: #303133;
}

.selector-summary span {
  color: #909399;
  font-size: 13px;
}

.selector-content {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  align-content: start;
  gap: 10px;
  margin: 8px 20px 18px;
  padding-right: 4px;
  overflow-y: auto;
}

.option-item {
  display: flex;
  align-items: center;
  gap: 9px;
  min-height: 38px;
  padding: 8px 10px;
  overflow: hidden;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  color: #606266;
  background: #fff;
  cursor: pointer;
  text-align: left;
}

.option-item:hover,
.option-item.selected {
  border-color: #409eff;
  color: #409eff;
  background: #ecf5ff;
}

.option-check {
  display: grid;
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
  place-items: center;
  border: 1px solid #c0c4cc;
  border-radius: 3px;
  color: #fff;
  font-size: 12px;
}

.selected .option-check {
  border-color: #409eff;
  background: #409eff;
}

.empty-state {
  grid-column: 1 / -1;
  padding: 72px 0;
  color: #909399;
  text-align: center;
}

.indice-error {
  color: #d03050;
}

.selector-footer {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  align-items: center;
  padding: 14px 20px;
  border-top: 1px solid #ebeef5;
}

.footer-button {
  min-width: 72px;
  height: 34px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  color: #606266;
  background: #fff;
  cursor: pointer;
}

.select-all-button {
  justify-self: start;
}

.confirm-button {
  justify-self: center;
  border-color: #409eff;
  color: #fff;
  background: #409eff;
}

.clear-button {
  justify-self: end;
}

.filter-selector-enter-active,
.filter-selector-leave-active {
  transition: opacity 160ms ease;
}

.filter-selector-enter-active .selector-dialog,
.filter-selector-leave-active .selector-dialog {
  transition: transform 160ms ease;
}

.filter-selector-enter-from,
.filter-selector-leave-to {
  opacity: 0;
}

.filter-selector-enter-from .selector-dialog,
.filter-selector-leave-to .selector-dialog {
  transform: translateY(10px) scale(0.98);
}

@media (max-width: 640px) {
  .selector-mask {
    padding: 12px;
  }

  .selector-dialog {
    height: calc(100vh - 24px);
  }

  .selector-content {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
