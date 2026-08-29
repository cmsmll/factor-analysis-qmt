<script setup lang="ts">
import { computed, h, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import {
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NEmpty,
  NSelect,
  NSpin,
  NStatistic,
  NTag,
  type DataTableColumns,
} from 'naive-ui'

import FactorRankChart from '@/components/visualization/FactorRankChart.vue'
import PortfolioNavChart from '@/components/visualization/PortfolioNavChart.vue'
import SectorPieChart from '@/components/visualization/SectorPieChart.vue'
import TurnoverBarChart from '@/components/visualization/TurnoverBarChart.vue'
import { loadCachedFilter } from '@/stores/mode1'
import { MODE2_STRATEGIES, PROFIT_MODE_KEY, uniqueDates, useMode2Store } from '@/stores/mode2'
import type { StockItem } from '@/types/mode2'

defineOptions({ name: 'Mode2Preview' })

const route = useRoute()
const router = useRouter()
const store = useMode2Store()
const {
  base,
  currentDate,
  currentStrategyKey,
  history,
  stockItems,
  historyLoading,
  selectLoading,
  historyError,
  selectError,
} = storeToRefs(store)

const currentStrategy = computed(
  () => MODE2_STRATEGIES.find((strategy) => strategy.key === currentStrategyKey.value) ?? MODE2_STRATEGIES[0]!,
)

function backToList(): void {
  void router.push('/mode2')
}

function onStFilter(field: 'filter_bz' | 'filter_st', value: boolean): void {
  void store.setStFilter(field, value)
}

// 独立页直达/刷新：从持久化过滤恢复股票池与收益模式（对齐 mode1 预览的参数恢复）
async function ensureContext(): Promise<void> {
  if (!base.value.start || !base.value.end) {
    const cached = loadCachedFilter()
    if (cached?.start && cached?.end) {
      await store.applyPool({
        start: cached.start,
        end: cached.end,
        filter_bz: cached.filter_bz ?? false,
        filter_st: cached.filter_st ?? false,
        sector: cached.sector ?? [],
        indice: cached.indice ?? [],
      })
    }
  }
  const savedMode = Number(localStorage.getItem(PROFIT_MODE_KEY) ?? '')
  if (Number.isInteger(savedMode) && savedMode >= 1 && savedMode <= 4) {
    await store.applyProfitMode(savedMode as 1 | 2 | 3 | 4)
  }
}

// 预览路由参数（策略 id）→ 恢复上下文并加载该策略回测/名单（结果按 id 缓存）
watch(
  () => route.params.id,
  async (id) => {
    if (!id || route.name !== 'mode2-preview') return
    const strategyId = String(id)
    if (!MODE2_STRATEGIES.some((strategy) => strategy.key === strategyId)) {
      void router.replace('/mode2')
      return
    }
    await ensureContext()
    await store.selectStrategy(strategyId)
  },
  { immediate: true },
)

const dateOptions = computed(() =>
  history.value ? uniqueDates(history.value.datetime).map((date) => ({ label: date, value: date })) : [],
)

// 名单日期变更 → 重新加载该日名单
watch(currentDate, (date) => {
  if (date) void store.loadSelect(date)
})

const stats = computed(() => history.value?.stats ?? null)

const previewAvgCount = computed(() => {
  const counts = history.value?.count.slice(1) ?? []
  return counts.length ? counts.reduce((sum, value) => sum + value, 0) / counts.length : 0
})

function formatFactor(value: number): string {
  const abs = Math.abs(value)
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)} 亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)} 万`
  return value.toFixed(2)
}

const stockColumns: DataTableColumns<StockItem> = [
  { title: '#', key: 'rank', width: 48, render: (_row, index) => index + 1 },
  { title: '代码', key: 'code', width: 80 },
  { title: '名称', key: 'name', width: 110 },
  { title: '收盘价', key: 'factor', render: (row) => formatFactor(row.factor) },
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
</script>

<template>
  <div class="preview-layout">
    <div class="preview-toolbar">
      <NButton size="small" @click="backToList">← 返回列表</NButton>
      <span class="preview-title">{{ currentStrategy.name }} · {{ currentStrategy.desc }}</span>
    </div>

    <NSpin :show="historyLoading">
      <div v-if="historyError" class="error-tip">{{ historyError }}</div>
      <template v-else-if="history">
        <NCard title="回测统计" size="small" class="stats-card">
          <div class="stats-row">
            <NStatistic label="总收益" :value="(stats?.total_profit ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="年化收益" :value="(stats?.annualized ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="最大回撤" :value="(stats?.max_drawdown ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="胜率" :value="(stats?.win_rate ?? 0) * 100" precision="2" suffix="%" />
            <NStatistic label="平均入选数" :value="previewAvgCount" precision="0" />
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
      <NEmpty v-else description="暂无回测数据" class="empty-block" />
    </NSpin>

    <NCard :title="`${currentStrategy.name}名单（${currentStrategy.desc}）`" size="small" class="list-card">
      <div class="list-toolbar">
        <span class="list-date-label">名单日期</span>
        <NSelect v-model:value="currentDate" :options="dateOptions" class="date-select" />
        <span class="list-date-label">过滤ST</span>
        <NCheckbox :checked="base.filter_st" @update:checked="onStFilter('filter_st', $event)" />
        <span class="list-date-label">过滤北证</span>
        <NCheckbox :checked="base.filter_bz" @update:checked="onStFilter('filter_bz', $event)" />
        <span v-if="selectLoading" class="loading-tip">加载中…</span>
      </div>
      <NSpin :show="selectLoading">
        <div v-if="selectError" class="error-tip">{{ selectError }}</div>
        <template v-else-if="stockItems.length">
          <NDataTable
            :columns="stockColumns"
            :data="stockItems"
            :row-key="(row) => row.code"
            size="small"
            class="stock-table"
          />
          <div class="list-charts">
            <NCard title="行业/指数分布" size="small">
              <SectorPieChart :items="stockItems" />
            </NCard>
            <NCard title="收盘价排名" size="small">
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
.preview-layout {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding: 32px;
  max-width: 1440px;
  margin: 0 auto;
  overflow-y: auto;
}

.preview-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

.preview-title {
  font-size: 14px;
  color: #606266;
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
