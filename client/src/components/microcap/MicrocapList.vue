<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useRoute, useRouter } from 'vue-router'
import UiCard from '@/components/ui/UiCard.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiSelect from '@/components/ui/UiSelect.vue'
import UiSpin from '@/components/ui/UiSpin.vue'
import UiTable, { type UiTableColumn } from '@/components/ui/UiTable.vue'
import UiTag from '@/components/ui/UiTag.vue'
import FactorRankChart from '@/components/visualization/FactorRankChart.vue'
import SectorPieChart from '@/components/visualization/SectorPieChart.vue'
import { uniqueDates, useMode2Store } from '@/stores/mode2'
import type { StockItem } from '@/types/mode2'

defineOptions({ name: 'MicrocapList' })

/** 微盘股名单明细：日期选择 + ST/北证过滤 + 名单表格 + 分布/排名图（mode2 第三级明细页内容）。 */
const store = useMode2Store()
const { base, currentDate, history, stockItems, selectLoading, selectError } = storeToRefs(store)
// 事件回调内不可 useRoute()/useRouter()(无组件注入上下文),须在 setup 顶层取用
const route = useRoute()
const router = useRouter()

const strategy = computed(() => store.currentStrategy)

const dateOptions = computed(() =>
  history.value ? uniqueDates(history.value.datetime).map((date) => ({ label: date, value: date })) : [],
)

// 名单日期变更 → 重新加载该日名单
watch(currentDate, (date) => {
  if (date) void store.loadSelect(date)
})

// 用户主动切换名单日期 → 同步到 URL query（store 内部赋值（默认日期/query 读入）不写回）
function onDateChange(date: string | number | null): void {
  if (typeof date !== 'string' || !date) return
  if (route.query.date === date) return
  void router.replace({ query: { ...route.query, date } })
}

function onStFilter(field: 'filter_bz' | 'filter_st', value: boolean): void {
  void store.setStFilter(field, value)
}

function formatFactor(value: number): string {
  const abs = Math.abs(value)
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)} 亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)} 万`
  return value.toFixed(2)
}

const expandedKeys = ref<Array<string | number>>([])

function onRowClick(row: StockItem): void {
  const key = row.code
  expandedKeys.value = expandedKeys.value.includes(key)
    ? expandedKeys.value.filter((item) => item !== key)
    : [...expandedKeys.value, key]
}

// 整行鼠标悬浮显示小手
function rowProps(row: StockItem) {
  return {
    style: { cursor: 'pointer' },
    onClick: () => onRowClick(row),
  }
}

const columns: UiTableColumn<StockItem>[] = [
  { title: '#', key: 'rank', width: 70, render: (_row, index) => index + 1 },
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
        row.tags.map((tag) => h(UiTag, { size: 'small' }, () => tag)),
      ),
  },
  {
    type: 'expand',
    width: 40,
    renderExpand: (row: StockItem) =>
      h('div', { class: 'expand-row' }, [
        `开盘 ${row.open.toFixed(2)}｜最高 ${row.high.toFixed(2)}｜最低 ${row.low.toFixed(2)}｜收盘 ${row.close.toFixed(2)}｜`,
        `成交量 ${formatFactor(row.volume)}｜成交额 ${formatFactor(row.amount)}｜换手率 ${(row.turnover * 100).toFixed(2)}%`,
      ]),
  },
]
</script>

<template>
  <UiCard :title="`${strategy.name}名单（${strategy.desc}）`" size="small" class="list-card">
    <div class="list-toolbar">
      <span class="list-date-label">名单日期</span>
      <UiSelect
        v-model:value="currentDate"
        :options="dateOptions"
        class="date-select"
        @update:value="onDateChange"
      />
      <span class="list-date-label">过滤ST</span>
      <UiCheckbox :checked="base.filter_st" @update:checked="onStFilter('filter_st', $event)" />
      <span class="list-date-label">过滤北证</span>
      <UiCheckbox :checked="base.filter_bz" @update:checked="onStFilter('filter_bz', $event)" />
      <span v-if="selectLoading" class="loading-tip">加载中…</span>
    </div>
    <UiSpin :show="selectLoading">
      <div v-if="selectError" class="error-tip">{{ selectError }}</div>
      <template v-else-if="stockItems.length">
        <UiTable
          v-model:expanded-row-keys="expandedKeys"
          :columns="columns"
          :data="stockItems"
          :row-key="(row) => row.code"
          :row-props="rowProps"
          size="small"
          class="stock-table"
        />
        <div class="list-charts">
          <UiCard title="行业/指数分布" size="small">
            <SectorPieChart :items="stockItems" />
          </UiCard>
          <UiCard title="收盘价排名" size="small">
            <FactorRankChart :items="stockItems" />
          </UiCard>
        </div>
      </template>
      <UiEmpty v-else description="该日无符合条件的股票" class="empty-block" />
    </UiSpin>
  </UiCard>
</template>

<style scoped>
.list-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.list-date-label {
  font-size: 13px;
  color: var(--ui-text-regular, #606266);
}

.date-select {
  width: 140px;
}

.loading-tip {
  font-size: 12px;
  color: var(--ui-text-secondary, #909399);
}

.list-charts {
  display: flex;
  flex-direction: column;
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
  color: var(--ui-text-regular, #606266);
  line-height: 1.8;
}

.error-tip {
  padding: 12px;
  color: var(--ui-color-danger, #d03050);
  font-size: 13px;
}

.empty-block {
  padding: 32px 0;
}

/* 表头吸顶（与 market/明细页一致）：本地表格无滚动容器，th sticky 相对外层滚动 */
.list-card :deep(.ui-table__th) {
  position: sticky;
  top: 0;
  z-index: 10;
  background: var(--ui-bg-header, #fafafa);
}

</style>
