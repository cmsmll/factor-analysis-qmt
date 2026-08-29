<script setup lang="ts">
import { computed, h, onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import {
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NDatePicker,
  NEmpty,
  NForm,
  NFormItem,
  NInputNumber,
  NSelect,
  NSpin,
  NStatistic,
  NTag,
  type DataTableColumns,
} from 'naive-ui'

import { fetchIndices, fetchSectors } from '@/api/mode1'
import FactorRankChart from '@/components/visualization/FactorRankChart.vue'
import PortfolioNavChart from '@/components/visualization/PortfolioNavChart.vue'
import SectorPieChart from '@/components/visualization/SectorPieChart.vue'
import TurnoverBarChart from '@/components/visualization/TurnoverBarChart.vue'
import { uniqueDates, useMode2Store } from '@/stores/mode2'
import { useGlobalFilterSelectorStore } from '@/stores/globalFilterSelector'
import type { Mode2Field, Mode2FilterType, StockItem } from '@/types/mode2'

defineOptions({ name: 'Mode2Select' })

const store = useMode2Store()
const filterSelector = useGlobalFilterSelectorStore()
const {
  field,
  direction,
  filterType,
  threshold,
  selectN,
  profitMode,
  currentDate,
  history,
  stockItems,
  historyLoading,
  selectLoading,
  historyError,
  selectError,
} = storeToRefs(store)

// base 为 reactive 对象，直接取 store 属性（storeToRefs 会包成 Ref）。
const base = store.base

const sectorOptions = ref<string[]>([])
const indiceOptions = ref<string[]>([])

const fieldOptions = [
  { label: '收盘价', value: 'Close' },
  { label: '股息率', value: 'DividendYield' },
  { label: '总市值', value: 'TotalMarket' },
]
const directionOptions = [
  { label: '升序（小 → 大）', value: 'Asc' },
  { label: '降序（大 → 小）', value: 'Desc' },
]
const filterOptions: { label: string; value: Mode2FilterType }[] = [
  { label: '不过滤', value: 'None' },
  { label: '小于阈值', value: 'Less' },
  { label: '大于阈值', value: 'Greater' },
  { label: '等于阈值', value: 'Equal' },
]
const profitModeOptions = [
  { label: '1 · 次日收盘收益', value: 1 },
  { label: '2 · 次日日内收益', value: 2 },
  { label: '3 · 次日开盘买 T+2 开盘卖', value: 3 },
  { label: '4 · 次日开盘买 T+2 收盘卖', value: 4 },
]

const dateOptions = computed(() =>
  history.value ? uniqueDates(history.value.datetime).map((date) => ({ label: date, value: date })) : [],
)

const range = computed<[string, string] | null>(() =>
  base.start && base.end ? [base.start, base.end] : null,
)

// NDatePicker 的 value 类型不导出且不含字符串元组，经 never 桥接。
const pickerValue = computed(() => range.value as never)

function onRangeChange(value: unknown): void {
  const pair = value as [string, string] | null
  if (pair) {
    base.start = pair[0]
    base.end = pair[1]
  }
}

// 参数变更 → 防抖刷新回测（内部联动刷新当前日期名单，D8）。
let refreshTimer: ReturnType<typeof setTimeout> | undefined
function scheduleRefresh(): void {
  clearTimeout(refreshTimer)
  refreshTimer = setTimeout(() => {
    void store.loadHistory()
  }, 300)
}
watch([field, direction, filterType, threshold, selectN, profitMode], scheduleRefresh)
watch(base, scheduleRefresh, { deep: true })
watch(currentDate, (date) => {
  if (date) void store.loadSelect(date)
})

const stats = computed(() => history.value?.stats ?? null)

// 平均入选数：排除首点基线（D6，前端由 count 计算）。
const avgCount = computed(() => {
  const counts = history.value?.count.slice(1) ?? []
  return counts.length ? counts.reduce((sum, value) => sum + value, 0) / counts.length : 0
})

function percent(value: number | undefined, digits = 2): string {
  return value === undefined ? '-' : `${(value * 100).toFixed(digits)}%`
}

function formatFactor(value: number): string {
  const abs = Math.abs(value)
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)} 亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)} 万`
  return value.toFixed(2)
}

const columns: DataTableColumns<StockItem> = [
  { title: '#', key: 'rank', width: 48, render: (_row, index) => index + 1 },
  { title: '代码', key: 'code', width: 80 },
  { title: '名称', key: 'name', width: 110 },
  { title: '因子值', key: 'factor', render: (row) => formatFactor(row.factor) },
  {
    title: '涨跌幅',
    key: 'change_percent',
    width: 90,
    render: (row) => `${row.change_percent.toFixed(2)}%`,
  },
  { title: 'ST', key: 'is_st', width: 56, render: (row) => (row.is_st ? '是' : '否') },
  { title: '交易所', key: 'exchange', width: 70 },
  {
    title: '行业/指数',
    key: 'tags',
    render: (row) =>
      h(
        'div',
        { class: 'tag-cell' },
        row.tags.map((tag) => h(NTag, { size: 'small', bordered: false }, { default: () => tag })),
      ),
  },
  {
    type: 'expand',
    renderExpand: (row: StockItem) =>
      h('div', { class: 'expand-row' }, [
        `开盘 ${row.open.toFixed(2)}｜最高 ${row.high.toFixed(2)}｜最低 ${row.low.toFixed(2)}｜收盘 ${row.close.toFixed(2)}｜`,
        `成交量 ${formatFactor(row.volume)}｜成交额 ${formatFactor(row.amount)}｜换手率 ${row.turnover.toFixed(2)}%`,
      ]),
  },
]

async function openSectorSelector(): Promise<void> {
  if (!sectorOptions.value.length) {
    try {
      sectorOptions.value = await fetchSectors()
    } catch {
      // 拉取失败时以空选项打开
    }
  }
  const result = await filterSelector.open({
    title: '选择行业板块',
    options: sectorOptions.value,
    selected: base.sector,
  })
  if (result) base.sector = result
}

async function openIndiceSelector(): Promise<void> {
  if (!indiceOptions.value.length) {
    try {
      indiceOptions.value = await fetchIndices()
    } catch {
      // 拉取失败时以空选项打开
    }
  }
  const result = await filterSelector.open({
    title: '选择指数列表',
    options: indiceOptions.value,
    selected: base.indice,
  })
  if (result) base.indice = result
}

onMounted(() => {
  void store.init()
})
</script>

<template>
  <div class="mode2-layout">
    <NCard title="选股参数" size="small" class="param-card">
      <NForm inline label-placement="left" size="small">
        <NFormItem label="因子字段">
          <NSelect v-model:value="field" :options="fieldOptions" class="param-select" />
        </NFormItem>
        <NFormItem label="排序方向">
          <NSelect v-model:value="direction" :options="directionOptions" class="param-select" />
        </NFormItem>
        <NFormItem label="过滤条件">
          <NSelect v-model:value="filterType" :options="filterOptions" class="param-select filter-type" />
          <NInputNumber
            v-if="filterType !== 'None'"
            v-model:value="threshold"
            :min="0"
            class="threshold-input"
            placeholder="阈值"
          />
        </NFormItem>
        <NFormItem label="选股数量">
          <NInputNumber v-model:value="selectN" :min="1" :max="100" class="num-input" />
        </NFormItem>
        <NFormItem label="收益模式">
          <NSelect v-model:value="profitMode" :options="profitModeOptions" class="param-select profit-mode" />
        </NFormItem>
        <NFormItem label="日期范围">
          <NDatePicker
            :value="pickerValue"
            type="daterange"
            value-format="yyyy-MM-dd"
            class="date-range"
            @update:value="onRangeChange"
          />
        </NFormItem>
        <NFormItem label="过滤北交所">
          <NCheckbox v-model:checked="base.filter_bz" />
        </NFormItem>
        <NFormItem label="过滤ST">
          <NCheckbox v-model:checked="base.filter_st" />
        </NFormItem>
        <NFormItem label="行业板块">
          <NButton size="small" class="selector-button" @click="openSectorSelector">
            {{ base.sector.length ? `已选 ${base.sector.length} 项` : '全部行业' }}
          </NButton>
        </NFormItem>
        <NFormItem label="指数列表">
          <NButton size="small" class="selector-button" @click="openIndiceSelector">
            {{ base.indice.length ? `已选 ${base.indice.length} 项` : '全部指数' }}
          </NButton>
        </NFormItem>
      </NForm>
    </NCard>

    <NSpin :show="historyLoading">
      <div v-if="historyError" class="error-tip">{{ historyError }}</div>
      <template v-else-if="history">
        <NCard title="回测统计" size="small" class="stats-card">
          <div class="stats-row">
            <NStatistic label="总收益" :value="(stats?.total_profit ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="年化收益" :value="(stats?.annualized ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="最大回撤" :value="(stats?.max_drawdown ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="胜率" :value="(stats?.win_rate ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="平均入选数" :value="avgCount" precision="1" />
          </div>
        </NCard>
        <div class="chart-grid">
          <NCard title="组合 / 基准净值" size="small" class="chart-card">
            <PortfolioNavChart
              :dates="history.datetime"
              :portfolio="history.portfolio"
              :benchmark="history.benchmark"
              @select-date="currentDate = $event"
            />
            <div class="chart-tip">点击曲线上的日期可联动下方名单；区间尾部 1-2 个交易日无未来收益数据。</div>
          </NCard>
          <NCard title="调仓换手率 / 入选数" size="small" class="chart-card">
            <TurnoverBarChart
              :dates="history.datetime"
              :turnover="history.turnover"
              :count="history.count"
            />
          </NCard>
        </div>
      </template>
      <NEmpty v-else description="请配置参数后查看回测结果" class="empty-block" />
    </NSpin>

    <NCard title="选股名单" size="small" class="list-card">
      <div class="list-toolbar">
        <span class="list-date-label">名单日期</span>
        <NSelect v-model:value="currentDate" :options="dateOptions" class="date-select" />
        <span v-if="selectLoading" class="loading-tip">加载中…</span>
      </div>
      <NSpin :show="selectLoading">
        <div v-if="selectError" class="error-tip">{{ selectError }}</div>
        <template v-else-if="stockItems.length">
          <NDataTable
            :columns="columns"
            :data="stockItems"
            :row-key="(row) => row.code"
            size="small"
            class="stock-table"
          />
          <div class="list-charts">
            <NCard title="行业/指数分布" size="small">
              <SectorPieChart :items="stockItems" />
            </NCard>
            <NCard title="因子值排名" size="small">
              <FactorRankChart :items="stockItems" />
            </NCard>
          </div>
        </template>
        <NEmpty v-else description="该日无符合条件的股票" class="empty-block" />
      </NSpin>
    </NCard>
  </div>
</template>

<style scoped>
.mode2-layout {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  max-width: 1440px;
  margin: 0 auto;
}

.param-card :deep(.n-form-item) {
  margin-bottom: 0;
}

.param-select {
  width: 140px;
}

.filter-type {
  width: 110px;
}

.profit-mode {
  width: 210px;
}

.threshold-input,
.num-input {
  width: 110px;
}

.date-range {
  width: 250px;
}

.selector-button {
  min-width: 112px;
  color: #409eff;
}

.stats-row {
  display: flex;
  flex-wrap: wrap;
  gap: 32px;
}

.chart-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.chart-card {
  min-width: 0;
}

.chart-tip {
  margin-top: 4px;
  font-size: 12px;
  color: #909399;
}

.list-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.list-date-label {
  font-size: 13px;
  color: #606266;
}

.date-select {
  width: 140px;
}

.loading-tip {
  font-size: 12px;
  color: #909399;
}

.list-charts {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-top: 12px;
}

.tag-cell {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.expand-row {
  font-size: 13px;
  color: #606266;
  line-height: 1.8;
}

.error-tip {
  padding: 12px;
  color: #d03050;
  font-size: 13px;
}

.empty-block {
  padding: 32px 0;
}
</style>
