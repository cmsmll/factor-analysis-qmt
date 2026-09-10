<script setup lang="ts">
import { computed, h, onMounted, ref, type VNode } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import UiButton from '@/components/ui/UiButton.vue'
import UiDatePicker from '@/components/ui/UiDatePicker.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiPagination from '@/components/ui/UiPagination.vue'
import UiTable, { type UiTableColumn } from '@/components/ui/UiTable.vue'

import RefreshIcon from '@/assets/icons/refresh.svg'
import { fetchMarketList } from '@/api/market'
import { fetchDataRange, fetchIndices, fetchSectors } from '@/api/mode1'
import type { MarketSnapshotRow } from '@/api/market'
import { useGlobalFilterSelectorStore } from '@/stores/globalFilterSelector'
import { useGlobalLoadingStore } from '@/stores/globalLoading'
import { useGlobalMessageStore } from '@/stores/globalMessage'
import { boardStepPath } from '@/utils/boardNav'

defineOptions({ name: 'MarketBoard' })

const route = useRoute()
const router = useRouter()
const globalLoading = useGlobalLoadingStore()
const globalMessage = useGlobalMessageStore()
const filterSelector = useGlobalFilterSelectorStore()

// ── 数据 ──
const rows = ref<MarketSnapshotRow[]>([])
const loading = ref(false)
const error = ref('')
const snapshotDate = ref('')
/** 全市场数据日期边界(毫秒,来自 /api/range;失败则日历不限界) */
const dataMinMs = ref<number | undefined>(undefined)
const dataMaxMs = ref<number | undefined>(undefined)
/** 快照日期选择（毫秒时间戳；缺省取列表返回的末交易日） */
const snapshotDay = ref<number | null>(null)
const searchInput = ref('')
const sectorSel = ref<string[]>([])
const indiceSel = ref<string[]>([])
const sectorOptionsCache = ref<string[] | undefined>(undefined)
const indiceOptionsCache = ref<string[] | undefined>(undefined)
const page = ref(1)
const pageSize = ref(20)
const pageSizeOptions = [10, 20, 50, 100]

// 排序状态（跨页全量排序）
const sortKey = ref<string | null>(null)
const sortOrder = ref<'ascend' | 'descend' | false>(false)

const HEADER_GRADIENT = 'linear-gradient(135deg, #0f4c5c 0%, #146a7a 45%, #1a8a9e 100%)'

// 看板三态循环：行情 → mode1 → mode2
function switchBoard(step: number): void {
  void router.push(boardStepPath(route.path, step))
}

async function loadList(): Promise<void> {
  loading.value = true
  error.value = ''
  const requested = toDateText(snapshotDay.value)
  try {
    const [data, range] = await Promise.all([
      globalLoading.run(() => fetchMarketList(requested)),
      fetchDataRange().catch(() => null),
    ])
    rows.value = data
    // 带日期请求:展示该快照日;缺省请求:记录末交易日并回填日期选择器
    snapshotDate.value = data[0]?.datetime ?? (requested ?? '')
    if (!requested) {
      if (snapshotDate.value) snapshotDay.value = parseDateText(snapshotDate.value)
    }
    if (range) {
      const min = parseDateText(range.min_date)
      const max = parseDateText(range.max_date)
      dataMinMs.value = min > 0 ? min : undefined
      dataMaxMs.value = max > 0 ? max : undefined
    }
    if (requested && data.length === 0) {
      error.value = `快照日 ${requested} 无行情数据（非交易日或数据缺失），请选择其他日期`
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    rows.value = []
  } finally {
    loading.value = false
  }
}

/** 'YYYY-MM-DD' -> 本地零点毫秒时间戳。 */
function parseDateText(text: string): number {
  const parsed = new Date(`${text}T00:00:00`)
  return Number.isNaN(parsed.getTime()) ? 0 : parsed.getTime()
}

/** 毫秒时间戳 -> 'YYYY-MM-DD'。 */
function toDateText(timestamp: number | null | undefined): string | undefined {
  if (timestamp === null || timestamp === undefined) return undefined
  const date = new Date(timestamp)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** 复位:回到末交易日(不带日期请求,由后端取 DF.end)并刷新。 */
function resetSnapshotDay(): void {
  snapshotDay.value = null
  void loadList()
}

/** 选择快照日:按该日期向后端重新获取行情列表。 */
function changeSnapshotDay(value: number | null): void {
  if (value === null) return
  snapshotDay.value = value
  page.value = 1
  void loadList()
}

async function selectSectors(): Promise<void> {
  try {
    if (!sectorOptionsCache.value) {
      sectorOptionsCache.value = await globalLoading.run(() => fetchSectors())
    }
    const result = await filterSelector.open({
      title: '行业板块',
      options: sectorOptionsCache.value,
      selected: sectorSel.value,
    })
    if (result && result.length) {
      sectorSel.value = result
      page.value = 1
    }
  } catch (err) {
    globalMessage.error(err instanceof Error ? err.message : '行业板块加载失败')
  }
}

async function selectIndices(): Promise<void> {
  try {
    if (!indiceOptionsCache.value) {
      indiceOptionsCache.value = await globalLoading.run(() => fetchIndices())
    }
    const result = await filterSelector.open({
      title: '指数列表',
      options: indiceOptionsCache.value,
      selected: indiceSel.value,
    })
    if (result && result.length) {
      indiceSel.value = result
      page.value = 1
    }
  } catch (err) {
    globalMessage.error(err instanceof Error ? err.message : '指数列表加载失败')
  }
}

function handleSorterChange(sorter: { columnKey?: string; order?: 'ascend' | 'descend' | false } | null): void {
  if (!sorter?.columnKey || !sorter.order) {
    sortKey.value = null
    sortOrder.value = false
    return
  }
  sortKey.value = sorter.columnKey
  sortOrder.value = sorter.order
  page.value = 1
}

// ── 过滤：股票搜索（代码/名称）+ 行业/指数并集 ──
const filtered = computed(() => {
  const keyword = searchInput.value.trim().toLowerCase()
  const pool = new Set([...sectorSel.value, ...indiceSel.value])
  return rows.value.filter((row) => {
    if (keyword && !row.code.toLowerCase().includes(keyword) && !row.name.toLowerCase().includes(keyword)) {
      return false
    }
    if (pool.size > 0 && !row.tags.some((tag) => pool.has(tag))) {
      return false
    }
    return true
  })
})

const total = computed(() => filtered.value.length)

// 排序取值器（全字段；序号按原位置 _pos）
const SORT_ACCESSORS: Record<string, (row: MarketSnapshotRow, pos: number) => number | string> = {
  no: (_row, pos) => pos,
  code: (row) => row.code,
  name: (row) => row.name,
  change_percent: (row) => row.change_percent,
  open: (row) => row.open,
  close: (row) => row.close,
  high: (row) => row.high,
  low: (row) => row.low,
  amount: (row) => row.amount,
  volume: (row) => row.volume,
  turnover_rate: (row) => row.turnover_rate,
}

function compareBy(accessor: (row: MarketSnapshotRow, pos: number) => number | string) {
  return (left: [MarketSnapshotRow, number], right: [MarketSnapshotRow, number]) => {
    const lv = accessor(left[0], left[1])
    const rv = accessor(right[0], right[1])
    if (typeof lv === 'string' || typeof rv === 'string') {
      return String(lv ?? '').localeCompare(String(rv ?? ''), 'zh-CN', {
        numeric: true,
        sensitivity: 'base',
      })
    }
    return Number(lv ?? 0) - Number(rv ?? 0)
  }
}

/** 全量排序（跨页）后分页。 */
const sortedPaged = computed(() => {
  const indexed = filtered.value.map((row, pos) => [row, pos] as [MarketSnapshotRow, number])
  if (sortKey.value && sortOrder.value) {
    const accessor = SORT_ACCESSORS[sortKey.value]
    if (accessor) {
      const direction = sortOrder.value === 'ascend' ? 1 : -1
      indexed.sort((left, right) => compareBy(accessor)(left, right) * direction)
    }
  }
  const start = (page.value - 1) * pageSize.value
  return indexed.slice(start, start + pageSize.value)
})

/** 点击行:新标签页打开该股 K线(列表页保持不动,日期参数随行带入)。 */
function openDetail(row: MarketSnapshotRow): void {
  const date = toDateText(snapshotDay.value) ?? snapshotDate.value
  const url = router.resolve({
    path: `/market/${row.code}`,
    query: date ? { date } : {},
  }).href
  window.open(url, '_blank', 'noopener')
}

// ── 格式化 ──
function money(value: number): string {
  const abs = Math.abs(value)
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)} 亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)} 万`
  return value.toFixed(2)
}

function hands(value: number): string {
  const hValue = value / 100 // 股 → 手
  const abs = Math.abs(hValue)
  if (abs >= 1e8) return `${(hValue / 1e8).toFixed(2)} 亿手`
  if (abs >= 1e4) return `${(hValue / 1e4).toFixed(2)} 万手`
  return `${hValue.toFixed(0)} 手`
}

function pctText(value: number): string {
  return `${value > 0 ? '+' : ''}${(value * 100).toFixed(2)}%`
}

// 红涨绿跌
const UP_COLOR = '#ef4444'
const DOWN_COLOR = '#22c55e'

function changeRender(row: MarketSnapshotRow): VNode {
  const color = row.change_percent >= 0 ? UP_COLOR : DOWN_COLOR
  return h('span', { style: { color } }, pctText(row.change_percent))
}

function numberRender(value: number): string {
  return value.toFixed(2)
}

interface SortableColumn {
  key: string
  sorter: boolean
  sortOrder?: 'ascend' | 'descend' | false
}

function withSort<T extends { key: string }>(column: T): T & SortableColumn {
  return {
    ...column,
    sorter: true,
    sortOrder: sortKey.value === column.key ? sortOrder.value : false,
  }
}

const columns = computed<UiTableColumn<MarketSnapshotRow>[]>(() => {
  const offset = (page.value - 1) * pageSize.value
  return [
    {
      title: '序号',
      key: 'no',
      width: 70,
      align: 'center',
      sorter: true,
      sortOrder: sortKey.value === 'no' ? sortOrder.value : false,
      render: (_row: MarketSnapshotRow, index: number) => String(offset + index + 1),
    },
    withSort({ title: '代码', key: 'code' }),
    withSort({ title: '名称', key: 'name' }),
    withSort({ title: '涨跌幅', key: 'change_percent', align: 'center', render: (row: MarketSnapshotRow) => changeRender(row) }),
    withSort({ title: '开盘价', key: 'open', render: (row: MarketSnapshotRow) => numberRender(row.open) }),
    withSort({ title: '收盘价', key: 'close', render: (row: MarketSnapshotRow) => numberRender(row.close) }),
    withSort({ title: '最高价', key: 'high', render: (row: MarketSnapshotRow) => numberRender(row.high) }),
    withSort({ title: '最低价', key: 'low', render: (row: MarketSnapshotRow) => numberRender(row.low) }),
    withSort({ title: '成交额', key: 'amount', render: (row: MarketSnapshotRow) => money(row.amount) }),
    withSort({ title: '成交量', key: 'volume', render: (row: MarketSnapshotRow) => hands(row.volume) }),
    withSort({ title: '换手率', key: 'turnover_rate', render: (row: MarketSnapshotRow) => pctText(row.turnover_rate) }),
  ]
})

const rowProps = (row: unknown) => {
  const item = row as MarketSnapshotRow
  return {
    style: { cursor: 'pointer' },
    onClick: () => openDetail(item),
  }
}

onMounted(() => {
  void loadList()
})
</script>

<template>
  <div class="market-layout">
    <!-- 标题：行情预览 + 左右切换三看板 -->
    <header class="page-header" :style="{ background: HEADER_GRADIENT }">
      <UiButton type="text" class="header-switch-btn" aria-label="上一看板" @click="switchBoard(-1)">
        <template #icon>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
            <path
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="48"
              d="M328 112L184 256l144 144"
            ></path>
          </svg>
        </template>
      </UiButton>
      <div class="header-content">
        <h1 class="page-title">行情预览</h1>
        <p class="page-subtitle">全部A股行情快照 · 末交易日 {{ snapshotDate || '—' }}</p>
      </div>
      <UiButton type="text" class="header-switch-btn" aria-label="下一看板" @click="switchBoard(1)">
        <template #icon>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
            <path
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="48"
              d="M184 112l144 144-144 144"
            ></path>
          </svg>
        </template>
      </UiButton>
    </header>

    <!-- 筛选条：股票搜索 + 行业/指数 + 重载 -->
    <div class="filter-bar">
      <div class="filter-form">
        <label class="filter-item">
          <span class="filter-item__label">股票搜索</span>
          <UiInput
            v-model:value="searchInput"
            placeholder="代码 / 名称"
            clearable
            size="small"
            style="width: 200px"
          />
        </label>
        <label class="filter-item">
          <span class="filter-item__label">行业板块</span>
          <UiButton size="small" class="selector-button" @click="selectSectors">
            {{ sectorSel.length ? `已选 ${sectorSel.length} 项` : '全部行业' }}
          </UiButton>
        </label>
        <label class="filter-item">
          <span class="filter-item__label">指数列表</span>
          <UiButton size="small" class="selector-button" @click="selectIndices">
            {{ indiceSel.length ? `已选 ${indiceSel.length} 项` : '全部指数' }}
          </UiButton>
        </label>
        <label class="filter-item">
          <span class="filter-item__label">行情日期</span>
          <UiDatePicker
            :value="snapshotDay"
            :disabled="loading"
            size="small"
            :clearable="false"
            :min="dataMinMs"
            :max="dataMaxMs"
            style="width: 150px"
            action-text="复位"
            @update:value="changeSnapshotDay"
            @action="resetSnapshotDay"
          />
        </label>
        <div class="filter-item reload-form-item">
          <UiButton
            type="primary"
            size="small"
            class="reload-btn"
            :loading="loading"
            @click="loadList"
          >
            <template #icon><img :src="RefreshIcon" alt="" class="reload-icon" /></template>
            重载
          </UiButton>
        </div>
      </div>
    </div>

    <p v-if="error" class="error-tip">{{ error }}</p>

    <!-- 列表：全字段可排序 -->
    <UiTable
      :columns="columns"
      :data="sortedPaged.map(([row]) => row)"
      :loading="loading"
      :bordered="true"
      :row-props="rowProps"
      size="small"
      class="market-table"
      @update:sorter="handleSorterChange"
    />
    <UiEmpty v-if="!loading && total === 0 && !error" description="没有匹配的股票" class="empty-block" />

    <!-- 分页 -->
    <div class="pagination-wrap">
      <UiPagination
        :page="page"
        :page-size="pageSize"
        :item-count="total"
        :page-sizes="pageSizeOptions"
        show-size-picker
        @update:page="(value: number) => (page = value)"
        @update:page-size="(size: number) => (pageSize = size)"
      />
    </div>

    <!-- 页脚 -->
    <footer class="page-footer">
      <span>行情预览 © 2026 · 共 {{ total }} 只</span>
    </footer>
  </div>
</template>

<style scoped>
.market-layout {
  display: flex;
  flex-direction: column;
  padding: 32px;
  gap: 24px;
  max-width: 1440px;
  margin: 0 auto;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-radius: 10px;
  overflow: hidden;
  padding: 28px 32px;
  box-shadow: 0 4px 20px rgba(15, 76, 92, 0.25);
}

.page-header .header-switch-btn {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.88);
  background: rgba(255, 255, 255, 0.1);
}

.page-header .header-switch-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.18);
}

.header-switch-btn svg {
  width: 22px;
  height: 22px;
}

.header-content {
  flex: 1;
  text-align: center;
  position: relative;
}

.page-title {
  margin: 0;
  font-size: 30px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 2px;
}

.page-subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.65);
  letter-spacing: 2px;
}

.filter-bar {
  background: var(--ui-bg-card, #fff);
  padding: 16px 20px;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.filter-form {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 20px;
}

.filter-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin: 0;
}

.filter-item__label {
  flex: 0 0 auto;
  color: var(--ui-text-label, #1f2225);
  font-size: var(--ui-font-base, 14px);
  white-space: nowrap;
}

.filter-form .selector-button {
  min-width: 112px;
  color: var(--ui-color-primary, #409eff);
}

.reload-form-item {
  margin-left: auto;
}

.reload-btn {
  min-width: 76px;
}

.reload-icon {
  width: 14px;
  height: 14px;
  filter: brightness(0) invert(1);
}

.market-table {
  background: var(--ui-bg-card, #fff);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

/* 与迁移前一致：小号表格但 14px 字号 + 12px 单元内边距（覆盖 UiTable small 默认值） */
.market-table :deep(.ui-table__table .ui-table__th),
.market-table :deep(.ui-table__table .ui-table__td) {
  padding: 12px;
  font-size: 14px;
}

.error-tip {
  color: var(--ui-color-danger, #d03050);
  padding: 8px 0;
}

.market-layout .empty-block {
  padding: 8px 0;
}

.pagination-wrap {
  display: flex;
  justify-content: center;
  padding: 4px 0;
}

.page-footer {
  display: flex;
  justify-content: center;
  padding: 12px 0;
  color: var(--ui-text-secondary, #909399);
  font-size: 13px;
}
</style>
