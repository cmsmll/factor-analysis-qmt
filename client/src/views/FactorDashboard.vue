<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import { NConfigProvider, NDataTable, NPagination, type DataTableColumns } from 'naive-ui'

import { useMode1Store } from '@/stores/mode1'
import type { Mode1Data, ModeRequest, Profit, ProfitMode } from '@/types/mode1'

defineOptions({ name: 'FactorDashboard' })

interface FactorRow {
  id: string
  sourceIndex: number
  index: number
  params: ModeRequest
  data?: Mode1Data
}

/** 看板固定过滤区下发的过滤状态（数据加载由 KanbanBoard 驱动）。 */
const props = defineProps<{
  searchKeyword: string
  profitMode: ProfitMode
  /** 列表刷新版本号：变化时重置分页 */
  revision: number
}>()

const router = useRouter()
const store = useMode1Store()
const { items, listLoading } = storeToRefs(store)

const page = ref(1)
const pageSize = ref(10)
const sortKey = ref<string | null>(null)
const sortOrder = ref<'ascend' | 'descend' | false>(false)

const filteredItems = computed(() => {
  const keyword = props.searchKeyword.trim().toLowerCase()
  const indexedItems = items.value.map((item, sourceIndex) => ({ item, sourceIndex }))

  return keyword
    ? indexedItems.filter(({ item }) => factorItemName(item).toLowerCase().includes(keyword))
    : indexedItems
})
const itemCount = computed(() => filteredItems.value.length)
const pageSizeOptions = computed(() => {
  const max = Math.ceil(itemCount.value / 10) * 10
  return Array.from({ length: max / 10 }, (_, index) => (index + 1) * 10)
})

const rows = computed<FactorRow[]>(() => {
  const offset = (page.value - 1) * pageSize.value
  const data = filteredItems.value.map(({ item, sourceIndex }) => ({
    id: item.args.base.id,
    sourceIndex,
    index: 0,
    params: item.args,
    data: item.data,
  }))

  if (sortKey.value && sortOrder.value) {
    const direction = sortOrder.value === 'ascend' ? 1 : -1
    data.sort((left, right) => compareRows(left, right, sortKey.value!) * direction)
  }

  return data.slice(offset, offset + pageSize.value).map((row, index) => ({
    ...row,
    index: offset + index + 1,
  }))
})

watch(
  () => props.searchKeyword,
  () => {
    page.value = 1
  },
)

watch(
  () => props.revision,
  () => {
    page.value = 1
  },
)

watch(itemCount, (count) => {
  const lastPage = Math.max(1, Math.ceil(count / pageSize.value))
  if (page.value > lastPage) page.value = lastPage
})

function handleSorterChange(
  sorter: { columnKey?: string; order?: 'ascend' | 'descend' | false } | null,
) {
  if (!sorter?.columnKey || !sorter.order) {
    sortKey.value = null
    sortOrder.value = false
    return
  }

  sortKey.value = sorter.columnKey
  sortOrder.value = sorter.order
}

function handlePageChange(value: number) {
  page.value = value
}

function handlePageSizeChange(value: number) {
  pageSize.value = value
  page.value = 1
}

function factorName(row: FactorRow): string {
  if (row.data?.name) return row.data.name
  return row.id
}

function factorInfo(row: FactorRow): string | undefined {
  return row.data?.info || undefined
}

function factorItemName(item: { args: ModeRequest; data?: Mode1Data }): string {
  return item.data?.name || item.args.base.id
}

function first<T>(values: readonly T[]): T | undefined {
  return values[0]
}

function last<T>(values: readonly T[]): T | undefined {
  return values.length > 0 ? values[values.length - 1] : undefined
}

function average(values: readonly number[] | undefined): number | undefined {
  if (!values || values.length === 0) return undefined
  const valid = values.filter(Number.isFinite)
  if (valid.length === 0) return undefined
  return valid.reduce((sum, value) => sum + value, 0) / valid.length
}

function profitGroups(row: FactorRow): Profit[] {
  if (!row.data) return []

  switch (props.profitMode) {
    case 2:
      return row.data.profit2
    case 3:
      return row.data.profit3
    case 4:
      return row.data.profit4
    default:
      return row.data.profit1
  }
}

function minAnnualized(row: FactorRow): number | undefined {
  return first(profitGroups(row))?.annualized_profit
}

function maxAnnualized(row: FactorRow): number | undefined {
  return last(profitGroups(row))?.annualized_profit
}

function minTotalProfit(row: FactorRow): number | undefined {
  return first(profitGroups(row))?.total_profit
}

function maxTotalProfit(row: FactorRow): number | undefined {
  return last(profitGroups(row))?.total_profit
}

function minTurnover(row: FactorRow): number | undefined {
  return average(first(row.data?.turnover_rate ?? []))
}

function maxTurnover(row: FactorRow): number | undefined {
  return average(last(row.data?.turnover_rate ?? []))
}

function sortableValue(row: FactorRow, key: string): number {
  const values: Record<string, number | undefined> = {
    min_year_rate: minAnnualized(row),
    max_year_rate: maxAnnualized(row),
    min_total_profit: minTotalProfit(row),
    max_total_profit: maxTotalProfit(row),
    min_turnover_rate: minTurnover(row),
    max_turnover_rate: maxTurnover(row),
  }
  return values[key] ?? 0
}

function compareRows(left: FactorRow, right: FactorRow, key: string): number {
  if (key === 'factor_name') {
    const result = factorName(left).localeCompare(factorName(right), 'zh-CN', {
      numeric: true,
      sensitivity: 'base',
    })
    return result || left.id.localeCompare(right.id)
  }

  return sortableValue(left, key) - sortableValue(right, key)
}

function formatPercent(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return '--'
  return `${(value * 100).toFixed(2)}%`
}

function rateColor(value: number | undefined): string {
  if (value === undefined || value === 0) return '#1f2225'
  return value > 0 ? '#d03050' : '#18a058'
}

function renderRateCell(value: number | undefined) {
  return h('span', { style: { color: rateColor(value) } }, formatPercent(value))
}

function renderTurnoverCell(value: number | undefined) {
  return h('span', formatPercent(value))
}

const columns: DataTableColumns<FactorRow> = [
  {
    title: '序号',
    key: 'index',
    align: 'center',
    width: 60,
  },
  {
    key: 'factor_name',
    title: '因子名称',
    align: 'center',
    sorter: true,
    width: 300,
    ellipsis: { tooltip: true },
    render(row) {
      return h(
        'span',
        {
          style: { color: 'rgb(30, 70, 125)', cursor: 'pointer' },
          title: factorInfo(row),
          onClick: () => {
            store.setCurr(items.value[row.sourceIndex] ?? null)
            void router.push({
              name: 'mode1-preview',
              params: { id: row.id },
            })
          },
        },
        factorName(row),
      )
    },
  },
  {
    key: 'min_year_rate',
    title: '最小分位数\n年化收益率',
    align: 'center',
    sorter: true,
    width: 200,
    render: (row) => renderRateCell(minAnnualized(row)),
  },
  {
    key: 'max_year_rate',
    title: '最大分位数\n年化收益率',
    align: 'center',
    sorter: true,
    width: 200,
    render: (row) => renderRateCell(maxAnnualized(row)),
  },
  {
    key: 'min_total_profit',
    title: '最小分位数\n总收益',
    align: 'center',
    sorter: true,
    width: 180,
    render: (row) => renderRateCell(minTotalProfit(row)),
  },
  {
    key: 'max_total_profit',
    title: '最大分位数\n总收益',
    align: 'center',
    sorter: true,
    width: 180,
    render: (row) => renderRateCell(maxTotalProfit(row)),
  },
  {
    key: 'min_turnover_rate',
    title: '最小分位数\n换手率',
    align: 'center',
    sorter: true,
    width: 170,
    render: (row) => renderTurnoverCell(minTurnover(row)),
  },
  {
    key: 'max_turnover_rate',
    title: '最大分位数\n换手率',
    align: 'center',
    sorter: true,
    width: 170,
    render: (row) => renderTurnoverCell(maxTurnover(row)),
  },
]

const rowKey = (row: FactorRow) => `${row.id}:${row.sourceIndex}`
</script>

<template>
  <NConfigProvider>
    <div class="factorKanban-layout">
      <!-- 表格区域 -->
      <NDataTable
        :columns="columns"
        :data="rows"
        :row-key="rowKey"
        :loading="listLoading"
        :bordered="true"
        :single-line="true"
        :pagination="false"
        :style="{
          'white-space': 'pre-wrap',
          '--n-font-size': '14px',
          '--n-th-padding': '12px',
          '--n-td-padding': '12px',
        }"
        size="small"
        class="factor-table"
        @update:sorter="handleSorterChange"
      />

      <!-- 分页 -->
      <div class="pagination-wrap">
        <NPagination
          :page="page"
          :page-size="pageSize"
          :item-count="itemCount"
          :page-sizes="pageSizeOptions"
          show-size-picker
          @update:page="handlePageChange"
          @update:page-size="handlePageSizeChange"
        />
      </div>

      <!-- 页尾 -->
      <footer class="page-footer">
        <span>因子看板 &copy; 2026</span>
      </footer>
    </div>
  </NConfigProvider>
</template>

<style scoped>
.factorKanban-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 24px;
}

.factor-table {
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  padding: 0;
  flex: 1;
}

.pagination-wrap {
  display: flex;
  justify-content: center;
  padding: 4px 0;
}

.pagination-wrap :deep(.n-pagination-item--button) {
  --n-button-color: #fff;
}

.page-footer {
  display: flex;
  justify-content: center;
  padding: 12px 0;
  color: #999;
  font-size: 13px;
}
</style>
