<script setup lang="ts">
import { computed, h, onMounted, reactive, ref, watch, type VNode } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import {
  NButton,
  NConfigProvider,
  NDataTable,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NPagination,
  NSelect,
  type DataTableColumns,
} from 'naive-ui'

import RefreshIcon from '@/assets/icons/refresh.svg'
import { fetchIndices, fetchSectors } from '@/api/mode1'
import { createModeFilter, loadCachedFilter, useMode1Store } from '@/stores/mode1'
import { MODE2_STRATEGIES, PROFIT_MODE_KEY, useMode2Store } from '@/stores/mode2'
import { useGlobalFilterSelectorStore } from '@/stores/globalFilterSelector'
import { useGlobalLoadingStore } from '@/stores/globalLoading'
import { useGlobalMessageStore } from '@/stores/globalMessage'
import type { Mode1Data, ModeFilter, ModeRequest, Period, Profit, ProfitMode } from '@/types/mode1'
import type { Mode2History, Mode2Strategy } from '@/types/mode2'

defineOptions({ name: 'KanbanBoard' })

// ── mode 渲染配置（统一组件：标题/过滤/列表三部分，差异全部收敛到配置）──
interface ListColumnConfig {
  key: string
  title: string
  width?: number
  align?: 'left' | 'center' | 'right'
  ellipsis?: boolean | { tooltip?: boolean }
  sorter?: boolean
  /** 排序取值：数值列返回 number，文本列返回 string（缺省取 row[key]） */
  sortValue?: (row: ListRow) => number | string | undefined
  /** 单元格渲染（缺省按 sortValue 结果展示） */
  render?: (row: ListRow, index: number) => VNode | string
}

interface ListModeConfig {
  mode: 'mode1' | 'mode2'
  header: { title: string; subtitle: string; gradient: string }
  search: { label: string; placeholder: string }
  columns: ListColumnConfig[]
  footer: string
}

interface Mode1Row {
  id: string
  sourceIndex: number
  index: number
  params: ModeRequest
  data?: Mode1Data
  _pos: number
  factor_name: string
  min_year_rate?: number
  max_year_rate?: number
  min_total_profit?: number
  max_total_profit?: number
  min_turnover_rate?: number
  max_turnover_rate?: number
}

interface Mode2Row {
  strategy: Mode2Strategy
  data?: Mode2History
  _pos: number
  name: string
  desc: string
  total_profit?: number
  annualized?: number
  max_drawdown?: number
  win_rate?: number
  avg_count?: number
}

type ListRow = Mode1Row | Mode2Row

const route = useRoute()
const router = useRouter()
const store = useMode1Store()
const mode2Store = useMode2Store()
const globalLoading = useGlobalLoadingStore()
const globalMessage = useGlobalMessageStore()
const filterSelector = useGlobalFilterSelectorStore()
const { periods, items, periodLoading, listLoading, listError, periodError } = storeToRefs(store)
const { strategyData, statsLoading: mode2StatsLoading } = storeToRefs(mode2Store)
const { visible: globalLoadingVisible } = storeToRefs(globalLoading)

// ── 看板由路由决定：/mode1 = 模式一，/mode2 = 模式二；左右按钮循环切换 ──
const isMode2 = computed(() => route.path.startsWith('/mode2'))
const config = computed<ListModeConfig>(() => (isMode2.value ? mode2Config : mode1Config))

function switchKanban(step: number) {
  const index = (Number(isMode2.value) + step + 2) % 2
  void router.push(index === 1 ? '/mode2' : '/mode1')
}

// ── 通用渲染器 ──
function rateColor(value: number | undefined): string {
  if (value === undefined || value === 0) return '#1f2225'
  return value > 0 ? '#d03050' : '#18a058'
}

function formatPercent(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return '--'
  return `${(value * 100).toFixed(2)}%`
}

function rateCell(value: number | undefined) {
  return h('span', { style: { color: rateColor(value) } }, formatPercent(value))
}

// 换手率列：后端已是百分比数值（如 2.29 即 2.29%），不再 ×100
function turnoverCell(value: number | undefined) {
  const text = value === undefined || !Number.isFinite(value) ? '--' : `${value.toFixed(2)}%`
  return h('span', text)
}

function integerCell(value: number | undefined) {
  return value === undefined ? h('span', '--') : h('span', String(value))
}

// ── mode1 配置 ──
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

function profitGroups(data: Mode1Data | undefined): Profit[] {
  if (!data) return []
  switch (filters.profitMode) {
    case 2:
      return data.profit2
    case 3:
      return data.profit3
    case 4:
      return data.profit4
    default:
      return data.profit1
  }
}

function factorName(item: { args: ModeRequest; data?: Mode1Data }): string {
  return item.data?.name || item.args.base.id
}

function mode1LinkCell(row: ListRow) {
  const item = row as Mode1Row
  return h(
    'span',
    {
      style: { color: 'rgb(30, 70, 125)', cursor: 'pointer' },
      title: item.data?.info,
      onClick: () => {
        store.setCurr(items.value[item.sourceIndex] ?? null)
        void router.push({ name: 'mode1-preview', params: { id: item.id } })
      },
    },
    item.factor_name,
  )
}

const mode1Config: ListModeConfig = {
  mode: 'mode1',
  header: {
    title: '因子看盘可视化显示',
    subtitle: '多因子量化分析平台',
    gradient: 'linear-gradient(135deg, #1a237e 0%, #283593 40%, #3949ab 100%)',
  },
  search: { label: '因子搜索', placeholder: '输入因子名称搜索' },
  footer: '因子看板 © 2026',
  columns: [
    {
      key: 'factor_name',
      title: '因子名称',
      align: 'center',
      sorter: true,
      width: 300,
      ellipsis: { tooltip: true },
      render: (row: ListRow) => mode1LinkCell(row),
    },
    {
      key: 'min_year_rate',
      title: '最小分位数\n年化收益率',
      align: 'center',
      sorter: true,
      width: 200,
      sortValue: (row) => (row as Mode1Row).min_year_rate,
      render: (row) => rateCell((row as Mode1Row).min_year_rate),
    },
    {
      key: 'max_year_rate',
      title: '最大分位数\n年化收益率',
      align: 'center',
      sorter: true,
      width: 200,
      sortValue: (row) => (row as Mode1Row).max_year_rate,
      render: (row) => rateCell((row as Mode1Row).max_year_rate),
    },
    {
      key: 'min_total_profit',
      title: '最小分位数\n总收益',
      align: 'center',
      sorter: true,
      width: 180,
      sortValue: (row) => (row as Mode1Row).min_total_profit,
      render: (row) => rateCell((row as Mode1Row).min_total_profit),
    },
    {
      key: 'max_total_profit',
      title: '最大分位数\n总收益',
      align: 'center',
      sorter: true,
      width: 180,
      sortValue: (row) => (row as Mode1Row).max_total_profit,
      render: (row) => rateCell((row as Mode1Row).max_total_profit),
    },
    {
      key: 'min_turnover_rate',
      title: '最小分位数\n换手率',
      align: 'center',
      sorter: true,
      width: 170,
      sortValue: (row) => (row as Mode1Row).min_turnover_rate,
      render: (row) => turnoverCell((row as Mode1Row).min_turnover_rate),
    },
    {
      key: 'max_turnover_rate',
      title: '最大分位数\n换手率',
      align: 'center',
      sorter: true,
      width: 170,
      sortValue: (row) => (row as Mode1Row).max_turnover_rate,
      render: (row) => turnoverCell((row as Mode1Row).max_turnover_rate),
    },
  ],
}

// ── mode2 配置 ──
function avgCount(data: Mode2History | undefined): number {
  const counts = data?.count.slice(1) ?? []
  return counts.length ? counts.reduce((sum, value) => sum + value, 0) / counts.length : 0
}

function mode2LinkCell(row: ListRow) {
  const item = row as Mode2Row
  return h(
    'span',
    {
      style: { color: 'rgb(30, 70, 125)', cursor: 'pointer' },
      onClick: () => void router.push(`/mode2/${item.strategy.key}`),
    },
    item.name,
  )
}

const mode2Config: ListModeConfig = {
  mode: 'mode2',
  header: {
    title: '因子选股可视化显示',
    subtitle: '微盘股 · 市值最小400只 → 收盘价最低80只',
    gradient: 'linear-gradient(135deg, #1b5e20 0%, #2e7d32 40%, #43a047 100%)',
  },
  search: { label: '策略搜索', placeholder: '输入策略名称搜索' },
  footer: '因子选股 © 2026',
  columns: [
    {
      key: 'name',
      title: '因子选股名称',
      align: 'center',
      sorter: true,
      width: 160,
      render: (row: ListRow) => mode2LinkCell(row),
    },
    {
      key: 'desc',
      title: '描述',
      sorter: true,
      ellipsis: { tooltip: true },
      sortValue: (row) => (row as Mode2Row).desc,
      render: (row) => (row as Mode2Row).desc,
    },
    {
      key: 'total_profit',
      title: '总收益',
      align: 'center',
      sorter: true,
      width: 110,
      sortValue: (row) => (row as Mode2Row).total_profit,
      render: (row) => rateCell((row as Mode2Row).total_profit),
    },
    {
      key: 'annualized',
      title: '年化收益',
      align: 'center',
      sorter: true,
      width: 110,
      sortValue: (row) => (row as Mode2Row).annualized,
      render: (row) => rateCell((row as Mode2Row).annualized),
    },
    {
      key: 'max_drawdown',
      title: '最大回撤',
      align: 'center',
      sorter: true,
      width: 110,
      sortValue: (row) => (row as Mode2Row).max_drawdown,
      render: (row) => rateCell((row as Mode2Row).max_drawdown),
    },
    {
      key: 'win_rate',
      title: '胜率',
      align: 'center',
      sorter: true,
      width: 90,
      sortValue: (row) => (row as Mode2Row).win_rate,
      render: (row) => rateCell((row as Mode2Row).win_rate),
    },
    {
      key: 'avg_count',
      title: '平均入选数',
      align: 'center',
      sorter: true,
      width: 130,
      sortValue: (row) => (row as Mode2Row).avg_count,
      render: (row) => integerCell((row as Mode2Row).avg_count),
    },
  ],
}

// ── 固定过滤区（标题/过滤/列表三部分中的「过滤」，两 mode 共用）──
const searchKeyword = ref('')
const strategyKeyword = ref('')
const searchInput = computed({
  get: () => (isMode2.value ? strategyKeyword.value : searchKeyword.value),
  set: (value: string) => {
    if (isMode2.value) strategyKeyword.value = value
    else searchKeyword.value = value
  },
})
let settingInitialPeriod = false
const listFilter = reactive<ModeFilter>({
  start: '',
  end: '',
  filter_bz: false,
  filter_st: false,
  sector: [],
  indice: [],
})
const filters = reactive({
  period: '',
  profitMode: 1 as ProfitMode,
})
const sectorOptions = ref<string[]>()
const indiceOptions = ref<string[]>()
// 列表刷新版本号：驱动分页重置
const listRevision = ref(0)

const periodOptions = computed(() =>
  periods.value.map((period) => ({ label: period.name, value: period.name })),
)
const profitModeOptions = [
  { label: '收益1：当天收盘买，第二天收盘卖', value: 1 },
  { label: '收益2：第二天开盘买，第二天收盘卖', value: 2 },
  { label: '收益3：第二天开盘买，第三天开盘卖', value: 3 },
  { label: '收益4：第二天开盘买，第三天收盘卖', value: 4 },
]

function cloneModeFilter(filter: ModeFilter): ModeFilter {
  return {
    ...filter,
    sector: [...filter.sector],
    indice: [...filter.indice],
  }
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}

async function reloadList(): Promise<void> {
  if (!listFilter.start || !listFilter.end) throw new Error('没有可用的时间周期配置')
  listRevision.value += 1
  await store.loadList(cloneModeFilter(listFilter))
  if (listError.value) throw new Error(listError.value)
}

async function reloadDashboard(): Promise<void> {
  try {
    await globalLoading.run(async () => {
      await reloadList()
    })
  } catch (error) {
    globalMessage.error(errorMessage(error, '获取模式一列表失败'))
  }
}

// 重载：模式一刷新因子列表，模式二强制刷新策略列表统计（绕过缓存）
function handleReload(): void {
  if (isMode2.value) {
    void mode2Store.loadListStats(true)
    return
  }
  void reloadDashboard()
}

function resetListFilter(period: Period): void {
  Object.assign(listFilter, createModeFilter(period))
}

function applyPeriodToFilter(period: Period): void {
  listFilter.start = period.start
  listFilter.end = period.end
}

function syncMode2(): void {
  void mode2Store.applyPool({
    start: listFilter.start,
    end: listFilter.end,
    filter_bz: listFilter.filter_bz,
    filter_st: listFilter.filter_st,
    sector: listFilter.sector,
    indice: listFilter.indice,
  })
}

async function loadPeriod(name: string) {
  const period = periods.value.find((item) => item.name === name)
  if (!period) return
  applyPeriodToFilter(period)
  if (!isMode2.value) {
    try {
      await globalLoading.run(async () => {
        await reloadList()
      })
    } catch (error) {
      globalMessage.error(errorMessage(error, '获取模式一列表失败'))
    }
  }
  syncMode2()
}

async function selectSectors(): Promise<void> {
  try {
    if (!sectorOptions.value) {
      await globalLoading.run(async () => {
        sectorOptions.value = await fetchSectors()
      })
    }
    const result = await filterSelector.open({
      title: '行业板块',
      options: sectorOptions.value ?? [],
      selected: listFilter.sector,
    })
    if (result) {
      listFilter.sector = result
      syncMode2()
    }
  } catch (error) {
    globalMessage.error(errorMessage(error, '行业板块加载失败'))
  }
}

async function selectIndices(): Promise<void> {
  try {
    if (!indiceOptions.value) {
      await globalLoading.run(async () => {
        indiceOptions.value = await fetchIndices()
      })
    }
    const result = await filterSelector.open({
      title: '指数列表',
      options: indiceOptions.value ?? [],
      selected: listFilter.indice,
    })
    if (result) {
      listFilter.indice = result
      syncMode2()
    }
  } catch (error) {
    globalMessage.error(errorMessage(error, '指数列表加载失败'))
  }
}

watch(
  () => filters.period,
  (name) => {
    if (!name || settingInitialPeriod) return
    void loadPeriod(name)
  },
  { flush: 'sync' },
)

// 收益模式变更 → 持久化 + 模式一表格列（profitGroups）与模式二回测/列表统计同步重算
watch(
  () => filters.profitMode,
  (mode) => {
    try {
      localStorage.setItem(PROFIT_MODE_KEY, String(mode))
    } catch {
      // localStorage full or unavailable
    }
    void mode2Store.applyProfitMode(mode as 1 | 2 | 3 | 4)
  },
)

async function initializeKanban(): Promise<void> {
  try {
    await globalLoading.run(async () => {
      await store.loadPeriods()
      if (periodError.value) throw new Error(periodError.value)

      const period = periods.value[0]
      if (!period) throw new Error('没有可用的时间周期配置')

      const cachedFilter = loadCachedFilter()
      if (cachedFilter) {
        settingInitialPeriod = true
        const matchingPeriod = periods.value.find(
          (p) => p.start === cachedFilter.start && p.end === cachedFilter.end,
        )
        filters.period = matchingPeriod?.name || period.name
        settingInitialPeriod = false
        Object.assign(listFilter, cachedFilter)
        if (!isMode2.value) await reloadList()
      } else {
        settingInitialPeriod = true
        filters.period = period.name
        settingInitialPeriod = false
        resetListFilter(period)
        if (!isMode2.value) await reloadList()
      }
    })
    const savedMode = Number(localStorage.getItem(PROFIT_MODE_KEY) ?? '')
    if (Number.isInteger(savedMode) && savedMode >= 1 && savedMode <= 4) {
      filters.profitMode = savedMode as ProfitMode
    }
    syncMode2()
  } catch (error) {
    if (!isMode2.value) {
      globalMessage.error(errorMessage(error, '获取模式一列表失败'))
    }
  }
}

onMounted(() => void initializeKanban())

// ── 列表（标题/过滤/列表三部分中的「列表」）──
const page = ref(1)
const pageSize = ref(10)
const sortKey = ref<string | null>(null)
const sortOrder = ref<'ascend' | 'descend' | false>(false)

const filteredMode1 = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase()
  const indexedItems = items.value.map((item, sourceIndex) => ({ item, sourceIndex }))
  return keyword
    ? indexedItems.filter(({ item }) => factorName(item).toLowerCase().includes(keyword))
    : indexedItems
})

const mode1Rows = computed<Mode1Row[]>(() =>
  filteredMode1.value.map(({ item, sourceIndex }, pos) => {
    const data = item.data
    const groups = profitGroups(data)
    return {
      id: item.args.base.id,
      sourceIndex,
      index: 0,
      params: item.args,
      data,
      _pos: pos,
      factor_name: factorName(item),
      min_year_rate: first(groups)?.annualized_profit,
      max_year_rate: last(groups)?.annualized_profit,
      min_total_profit: first(groups)?.total_profit,
      max_total_profit: last(groups)?.total_profit,
      min_turnover_rate: average(first(data?.turnover_rate ?? [])),
      max_turnover_rate: average(last(data?.turnover_rate ?? [])),
    }
  }),
)

const filteredMode2 = computed(() => {
  const keyword = strategyKeyword.value.trim().toLowerCase()
  return MODE2_STRATEGIES.filter(
    (strategy) => !keyword || strategy.name.toLowerCase().includes(keyword),
  )
})

const mode2Rows = computed<Mode2Row[]>(() =>
  filteredMode2.value.map((strategy, pos) => {
    const data = strategyData.value[strategy.key]
    return {
      strategy,
      data,
      _pos: pos,
      name: strategy.name,
      desc: strategy.desc,
      total_profit: data?.stats.total_profit,
      annualized: data?.stats.annualized,
      max_drawdown: data?.stats.max_drawdown,
      win_rate: data?.stats.win_rate,
      avg_count: data ? Math.round(avgCount(data)) : undefined,
    }
  }),
)

const allRows = computed<ListRow[]>(() => (isMode2.value ? mode2Rows.value : mode1Rows.value))
const itemCount = computed(() => allRows.value.length)
const pageSizeOptions = computed(() => {
  const max = Math.ceil(itemCount.value / 10) * 10
  return Array.from({ length: max / 10 }, (_, index) => (index + 1) * 10)
})
const tableLoading = computed(() => (isMode2.value ? mode2StatsLoading.value : listLoading.value))

const offset = computed(() => (page.value - 1) * pageSize.value)

function compareRows(left: ListRow, right: ListRow, column?: ListColumnConfig): number {
  if (!column) return 0
  const lv = column.sortValue
    ? column.sortValue(left)
    : (left as unknown as Record<string, unknown>)[column.key]
  const rv = column.sortValue
    ? column.sortValue(right)
    : (right as unknown as Record<string, unknown>)[column.key]
  if (typeof lv === 'string' || typeof rv === 'string') {
    return String(lv ?? '').localeCompare(String(rv ?? ''), 'zh-CN', {
      numeric: true,
      sensitivity: 'base',
    })
  }
  return Number(lv ?? 0) - Number(rv ?? 0)
}

const rows = computed<ListRow[]>(() => {
  const data = [...allRows.value]
  if (sortKey.value && sortOrder.value) {
    const column = config.value.columns.find((item) => item.key === sortKey.value)
    const direction = sortOrder.value === 'ascend' ? 1 : -1
    data.sort((left, right) => compareRows(left, right, column) * direction)
  }
  return data.slice(offset.value, offset.value + pageSize.value)
})

// 序号列：统一自动前置（按分页偏移连续编号），两 mode 都有且可排序（按原位置）
const columns = computed<DataTableColumns<ListRow>>(() => [
  {
    title: '序号',
    key: 'index',
    align: 'center',
    width: 80,
    sorter: true,
    sortValue: (row: ListRow) => row._pos,
    render: (_row: ListRow, index: number) =>
      h('span', { style: { 'white-space': 'nowrap' } }, String(offset.value + index + 1)),
  },
  ...config.value.columns.map((column) => ({
    ...column,
    render: column.render
      ? (row: ListRow, index: number) => column.render!(row, index)
      : (row: ListRow) => {
          const value = column.sortValue
            ? column.sortValue(row)
            : (row as unknown as Record<string, unknown>)[column.key]
          return value === undefined || value === null || value === ''
            ? '--'
            : String(value)
        },
  })),
])

const rowKey = (row: ListRow) =>
  isMode2.value
    ? (row as Mode2Row).strategy.key
    : `${(row as Mode1Row).id}:${(row as Mode1Row).sourceIndex}`

watch([searchKeyword, strategyKeyword], () => {
  page.value = 1
})

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
</script>

<template>
  <NConfigProvider>
    <div class="factorKanban-layout">
      <!-- 标题：背景 + 名称 + 注释 + 左右切换按钮 -->
      <header class="page-header" :style="{ background: config.header.gradient }">
        <NButton text circle class="header-switch-btn" aria-label="上一看板" @click="switchKanban(-1)">
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
          <h1 class="page-title">{{ config.header.title }}</h1>
          <p class="page-subtitle">{{ config.header.subtitle }}</p>
        </div>
        <NButton text circle class="header-switch-btn" aria-label="下一看板" @click="switchKanban(1)">
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

      <!-- 过滤：搜索栏名称随配置，其余两 mode 一致 -->
      <div class="filter-bar">
        <NForm layout="inline" label-placement="left" size="small">
          <NFormItem :label="config.search.label">
            <NInput
              v-model:value="searchInput"
              :placeholder="config.search.placeholder"
              clearable
              style="width: 180px"
            />
          </NFormItem>
          <NFormItem label="行业板块">
            <NButton size="small" class="selector-button" @click="selectSectors">
              {{ listFilter.sector.length ? `已选 ${listFilter.sector.length} 项` : '全部行业' }}
            </NButton>
          </NFormItem>
          <NFormItem label="指数列表">
            <NButton size="small" class="selector-button" @click="selectIndices">
              {{ listFilter.indice.length ? `已选 ${listFilter.indice.length} 项` : '全部指数' }}
            </NButton>
          </NFormItem>
          <NFormItem label="时间周期">
            <NSelect
              v-model:value="filters.period"
              :options="periodOptions"
              :loading="periodLoading && !globalLoadingVisible"
              :disabled="periodLoading || listLoading"
              style="width: 140px"
            />
          </NFormItem>
          <NFormItem label="收益模式">
            <NSelect
              v-model:value="filters.profitMode"
              :options="profitModeOptions"
              :consistent-menu-width="false"
              style="width: 260px"
            />
          </NFormItem>
          <NFormItem label="" class="reload-form-item">
            <NButton
              type="primary"
              color="#409eff"
              size="small"
              class="reload-btn"
              :loading="tableLoading && !globalLoadingVisible"
              :disabled="periodLoading || listLoading"
              @click="handleReload"
            >
              <template #icon><img :src="RefreshIcon" alt="" class="reload-icon" /></template>
              重载
            </NButton>
          </NFormItem>
        </NForm>
      </div>

      <!-- 列表：配置驱动列渲染；无内层滚动，页面全高由浏览器原生滚动 -->
      <NDataTable
        :columns="columns"
        :data="rows"
        :row-key="rowKey"
        :loading="tableLoading"
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
      <NEmpty
        v-if="isMode2 && !mode2StatsLoading && rows.length === 0"
        description="暂无选股策略"
        class="empty-block"
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

      <!-- 页脚 -->
      <footer class="page-footer">
        <span>{{ config.footer }}</span>
      </footer>
    </div>
  </NConfigProvider>
</template>

<style scoped>
/* 标题/过滤/列表三部分：不设高度与溢出，页面随内容增高，浏览器原生滚动（宽度对齐） */
.factorKanban-layout {
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
  box-shadow: 0 4px 20px rgba(26, 35, 126, 0.25);
  overflow: hidden;
  padding: 28px 32px;
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

.header-content::after {
  content: '';
  position: absolute;
  left: -80px;
  top: -50px;
  width: 140px;
  height: 140px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.04);
  pointer-events: none;
}

.header-content::before {
  content: '';
  position: absolute;
  right: -60px;
  bottom: -40px;
  width: 100px;
  height: 100px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.03);
  pointer-events: none;
}

.page-title {
  margin: 0;
  font-size: 30px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 2px;
  position: relative;
}

.page-subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.65);
  letter-spacing: 4px;
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

.factor-table {
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  padding: 0;
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

.empty-block {
  padding: 8px 0;
}
</style>
