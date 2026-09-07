<script setup lang="ts">
import { computed, h, onMounted, ref, type VNode } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  NButton,
  NConfigProvider,
  NDataTable,
  NDatePicker,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NPagination,
  dateZhCN,
  zhCN,
  type DataTableColumns,
} from 'naive-ui'

import RefreshIcon from '@/assets/icons/refresh.svg'
import { fetchMarketList } from '@/api/market'
import { fetchIndices, fetchSectors } from '@/api/mode1'
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
  try {
    const data = await globalLoading.run(() => fetchMarketList())
    rows.value = data
    snapshotDate.value = data[0]?.datetime ?? ''
    if (snapshotDate.value) snapshotDay.value = parseDateText(snapshotDate.value)
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

/** 复位：日期回到末交易日（列表不动）。 */
function resetSnapshotDay(): void {
  if (snapshotDate.value) snapshotDay.value = parseDateText(snapshotDate.value)
}

/** 日期仅作进入详情的 ±半年中心（列表仍为末交易日快照，不请求后端）。 */
function changeSnapshotDay(value: number | null): void {
  if (value === null) return
  snapshotDay.value = value
  page.value = 1
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

function openDetail(row: MarketSnapshotRow): void {
  const date = toDateText(snapshotDay.value) ?? snapshotDate.value
  void router.push({ path: `/market/${row.code}`, query: date ? { date } : {} })
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

const columns = computed<DataTableColumns<MarketSnapshotRow>>(() => {
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
    withSort({ title: '代码', key: 'code', width: 100 }),
    withSort({ title: '名称', key: 'name', width: 130 }),
    withSort({ title: '涨跌幅', key: 'change_percent', width: 110, align: 'right', render: (row: MarketSnapshotRow) => changeRender(row) }),
    withSort({ title: '开盘价', key: 'open', width: 100, align: 'right', render: (row: MarketSnapshotRow) => numberRender(row.open) }),
    withSort({ title: '收盘价', key: 'close', width: 100, align: 'right', render: (row: MarketSnapshotRow) => numberRender(row.close) }),
    withSort({ title: '最高价', key: 'high', width: 100, align: 'right', render: (row: MarketSnapshotRow) => numberRender(row.high) }),
    withSort({ title: '最低价', key: 'low', width: 100, align: 'right', render: (row: MarketSnapshotRow) => numberRender(row.low) }),
    withSort({ title: '成交额', key: 'amount', width: 120, align: 'right', render: (row: MarketSnapshotRow) => money(row.amount) }),
    withSort({ title: '成交量', key: 'volume', width: 120, align: 'right', render: (row: MarketSnapshotRow) => hands(row.volume) }),
    withSort({ title: '换手率', key: 'turnover_rate', width: 110, align: 'right', render: (row: MarketSnapshotRow) => pctText(row.turnover_rate) }),
  ] as DataTableColumns<MarketSnapshotRow>
})

const rowProps = (row: MarketSnapshotRow) => ({
  style: { cursor: 'pointer' },
  onClick: () => openDetail(row),
})

onMounted(() => {
  void loadList()
})
</script>

<template>
  <div class="market-layout">
    <!-- 标题：行情预览 + 左右切换三看板 -->
    <header class="page-header" :style="{ background: HEADER_GRADIENT }">
      <NButton text circle class="header-switch-btn" aria-label="上一看板" @click="switchBoard(-1)">
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
      </NButton>
      <div class="header-content">
        <h1 class="page-title">行情预览</h1>
        <p class="page-subtitle">全部A股行情快照 · 末交易日 {{ snapshotDate || '—' }}</p>
      </div>
      <NButton text circle class="header-switch-btn" aria-label="下一看板" @click="switchBoard(1)">
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
      </NButton>
    </header>

    <!-- 筛选条：股票搜索 + 行业/指数 + 重载 -->
    <div class="filter-bar">
      <NForm layout="inline" label-placement="left" size="small">
        <NFormItem label="股票搜索">
          <NInput
            v-model:value="searchInput"
            placeholder="代码 / 名称"
            clearable
            style="width: 200px"
          />
        </NFormItem>
        <NFormItem label="行业板块">
          <NButton size="small" class="selector-button" @click="selectSectors">
            {{ sectorSel.length ? `已选 ${sectorSel.length} 项` : '全部行业' }}
          </NButton>
        </NFormItem>
        <NFormItem label="指数列表">
          <NButton size="small" class="selector-button" @click="selectIndices">
            {{ indiceSel.length ? `已选 ${indiceSel.length} 项` : '全部指数' }}
          </NButton>
        </NFormItem>
        <NFormItem label="行情日期">
          <NConfigProvider :locale="zhCN" :date-locale="dateZhCN">
            <NDatePicker
              :value="snapshotDay"
              :disabled="loading"
              type="date"
              size="small"
              :clearable="false"
              to=".market-layout"
              style="width: 150px"
              @update:value="changeSnapshotDay"
            >
              <template #now>
                <NButton size="tiny" class="date-reset-btn" @click.stop.prevent="resetSnapshotDay">
                  复位
                </NButton>
              </template>
            </NDatePicker>
          </NConfigProvider>
        </NFormItem>
        <NFormItem label="" class="reload-form-item">
          <NButton
            type="primary"
            color="#409eff"
            size="small"
            class="reload-btn"
            :loading="loading"
            @click="loadList"
          >
            <template #icon><img :src="RefreshIcon" alt="" class="reload-icon" /></template>
            重载
          </NButton>
        </NFormItem>
      </NForm>
    </div>

    <p v-if="error" class="error-tip">{{ error }}</p>

    <!-- 列表：全字段可排序 -->
    <NDataTable
      :columns="columns"
      :data="sortedPaged.map(([row]) => row)"
      :loading="loading"
      :bordered="true"
      :single-line="true"
      :pagination="false"
      :row-props="rowProps"
      :style="{ '--n-font-size': '14px', '--n-th-padding': '12px', '--n-td-padding': '12px' }"
      size="small"
      class="market-table"
      @update:sorter="handleSorterChange"
    />
    <NEmpty v-if="!loading && total === 0 && !error" description="没有匹配的股票" class="empty-block" />

    <!-- 分页 -->
    <div class="pagination-wrap">
      <NPagination
        v-model:page="page"
        :page-size="pageSize"
        :item-count="total"
        :page-sizes="pageSizeOptions"
        show-size-picker
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

.header-switch-btn {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  color: rgba(255, 255, 255, 0.88);
  background: rgba(255, 255, 255, 0.1);
}

.header-switch-btn:hover {
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
  background: #fff;
  padding: 16px 20px;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.filter-bar :deep(.n-form) {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 20px;
}

.filter-bar :deep(.n-form-item) {
  margin-bottom: 0;
}

.filter-bar :deep(.n-form-item-feedback-wrapper) {
  display: none;
}

.selector-button {
  min-width: 112px;
  color: #409eff;
}

.date-reset-btn {
  margin-left: 8px;
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
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

/* 与 mode1 明细页一致：表头吸顶（解除 naive 滚动容器 overflow，th sticky 相对视口） */
.market-layout :deep(.n-data-table-base-table-body.n-scrollbar),
.market-layout :deep(.n-scrollbar-container),
.market-layout :deep(.n-data-table-wrapper) {
  overflow: visible;
}

.market-layout :deep(.n-data-table-thead .n-data-table-th) {
  position: sticky;
  top: 0;
  z-index: 10;
  background: rgb(250, 250, 252);
}

.error-tip {
  color: #d03050;
  padding: 8px 0;
}

.empty-block {
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
  color: #999;
  font-size: 13px;
}
</style>
