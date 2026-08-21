<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { NButton, NCheckbox, NModal, NRadioGroup, NRadio, NSpace } from 'naive-ui'
import { fetchIndiceHistory, fetchIndices } from '@/api/mode1'
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
}>()
const globalMessage = useGlobalMessageStore()
const sortMode = ref(0)
const showPicker = ref(false)
const indiceList = ref<string[]>([])
const indiceLoading = ref(false)
const indiceError = ref('')

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
    <VChart
      :ref="setChartRef"
      class="chart-body"
      :option="option"
      autoresize
      tabindex="0"
      @focus="handleChartFocus"
      @keydown="handleChartKeydown"
      @mousemove="handleChartMousemove"
    />
    <NModal
      v-model:show="showPicker"
      preset="card"
      title="指数同框"
      style="width: 360px"
      :bordered="false"
    >
      <NSpace vertical size="small">
        <div v-if="indiceLoading" class="indice-hint">加载中…</div>
        <div v-else-if="indiceError" class="indice-hint indice-error">{{ indiceError }}</div>
        <div v-else-if="indiceList.length === 0" class="indice-hint">暂无指数数据</div>
        <template v-else>
          <NCheckbox
            v-for="name in indiceList"
            :key="name"
            :checked="checkedIndices.has(name)"
            :loading="pendingIndices.has(name)"
            @update:checked="(checked: boolean) => toggleIndice(name, checked)"
          >
            {{ name }}
          </NCheckbox>
        </template>
      </NSpace>
    </NModal>
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

.indice-hint {
  font-size: 13px;
  color: rgb(118, 124, 130);
}

.indice-error {
  color: #d03050;
}
</style>
