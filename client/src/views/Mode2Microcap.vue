<script setup lang="ts">
import { computed, h } from 'vue'
import { useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import { NDataTable, NEmpty, NSpin, type DataTableColumns } from 'naive-ui'

import { MODE2_STRATEGIES, useMode2Store } from '@/stores/mode2'
import type { Mode2History, Mode2Strategy } from '@/types/mode2'

defineOptions({ name: 'Mode2Microcap' })

/** 策略搜索关键词（由看板固定过滤区下发，对标模式一的因子搜索）。 */
const props = defineProps<{
  strategyKeyword?: string
}>()

const router = useRouter()
const store = useMode2Store()
const { strategyData, statsLoading } = storeToRefs(store)

interface StrategyRow {
  strategy: Mode2Strategy
  data: Mode2History | undefined
}

const listRows = computed<StrategyRow[]>(() => {
  const keyword = (props.strategyKeyword ?? '').trim().toLowerCase()
  return MODE2_STRATEGIES.filter(
    (strategy) => !keyword || strategy.name.toLowerCase().includes(keyword),
  ).map((strategy) => ({
    strategy,
    data: strategyData.value[strategy.key],
  }))
})

function openStrategy(strategy: Mode2Strategy): void {
  void router.push(`/mode2/${strategy.key}`)
}

function avgCount(data: Mode2History | undefined): number {
  const counts = data?.count.slice(1) ?? []
  return counts.length ? counts.reduce((sum, value) => sum + value, 0) / counts.length : 0
}

function rateColor(value: number | undefined): string {
  if (value === undefined || value === 0) return '#1f2225'
  return value > 0 ? '#d03050' : '#18a058'
}

function rateCell(value: number | undefined) {
  const text = value === undefined ? '--' : `${(value * 100).toFixed(2)}%`
  return h('span', { style: { color: rateColor(value) } }, text)
}

const strategyColumns: DataTableColumns<StrategyRow> = [
  {
    title: '序号',
    key: 'index',
    align: 'center',
    width: 60,
    render: (_row, index) => index + 1,
  },
  {
    title: '因子选股名称',
    key: 'name',
    align: 'center',
    width: 160,
    render: (row) =>
      h(
        'span',
        {
          style: { color: 'rgb(30, 70, 125)', cursor: 'pointer' },
          onClick: () => openStrategy(row.strategy),
        },
        row.strategy.name,
      ),
  },
  {
    title: '描述',
    key: 'desc',
    ellipsis: { tooltip: true },
    render: (row) => row.strategy.desc,
  },
  {
    title: '总收益',
    key: 'total_profit',
    align: 'center',
    sorter: true,
    width: 110,
    render: (row) => rateCell(row.data?.stats.total_profit),
  },
  {
    title: '年化收益',
    key: 'annualized',
    align: 'center',
    sorter: true,
    width: 110,
    render: (row) => rateCell(row.data?.stats.annualized),
  },
  {
    title: '最大回撤',
    key: 'max_drawdown',
    align: 'center',
    sorter: true,
    width: 110,
    render: (row) => rateCell(row.data?.stats.max_drawdown),
  },
  {
    title: '胜率',
    key: 'win_rate',
    align: 'center',
    sorter: true,
    width: 90,
    render: (row) => rateCell(row.data?.stats.win_rate),
  },
  {
    title: '平均入选数',
    key: 'avg_count',
    align: 'center',
    sorter: true,
    width: 130,
    render: (row) => (row.data ? Math.round(avgCount(row.data)).toString() : '--'),
  },
]
</script>

<template>
  <div class="microcap-layout">
    <!-- 列表视图（初始）：与模式一列表一致，直接渲染表格 -->
    <NSpin :show="statsLoading">
      <NDataTable
        :columns="strategyColumns"
        :data="listRows"
        :row-key="(row) => row.strategy.key"
        :loading="statsLoading"
        size="small"
        class="strategy-table"
      />
      <NEmpty v-if="!statsLoading && listRows.length === 0" description="暂无选股策略" class="empty-block" />
    </NSpin>
  </div>
</template>

<style scoped>
.microcap-layout {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow-y: auto;
}

.strategy-table {
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.empty-block {
  padding: 32px 0;
}
</style>
