<script setup lang="ts">
import { computed, h, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import PageTitleBar from '@/components/common/PageTitleBar.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiDatePicker from '@/components/ui/UiDatePicker.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiRadio from '@/components/ui/UiRadio.vue'
import UiRadioGroup from '@/components/ui/UiRadioGroup.vue'
import UiSpin from '@/components/ui/UiSpin.vue'
import UiTable, { type UiTableColumn } from '@/components/ui/UiTable.vue'
import UiTabs from '@/components/ui/UiTabs.vue'
import { fetchDataRange, fetchMode1Detail } from '@/api/mode1'
import { loadPreviewParams, useMode1Store } from '@/stores/mode1'
import { useMode1PreviewStore } from '@/stores/mode1Preview'
import type { Mode1DetailRow, Mode1QuantileDay, ModeRequest } from '@/types/mode1'

defineOptions({ name: 'Mode1Detail' })

/** 表格行：分位内股票行 + 末尾特化「平均值」行。 */
interface TableRow {
  key: string
  isAvg: boolean
  code: string
  name: string
  close: number | null
  changePercent: number | null
  turnover: number | null
  factor: number | null
  /** 原始明细行（展开显示完整行情/财务用；平均值行为 null） */
  raw: Mode1DetailRow | null
}

/** 展开行集合（与 mode2 明细一致：点击行展开/收起）。 */
const expandedKeys = ref<Array<string | number>>([])

const route = useRoute()
const router = useRouter()
const mode1Store = useMode1Store()
const previewStore = useMode1PreviewStore()

const modeId = computed(() => {
  const value = route.params.id
  return Array.isArray(value) ? (value[0] ?? '') : (value ?? '')
})

const request = ref<ModeRequest>()
const day = ref<number | null>(null)
/** 日历可选边界(毫秒,全市场数据区间;失败不限界) */
const rangeMinMs = ref<number | undefined>(undefined)
const rangeMaxMs = ref<number | undefined>(undefined)
const quantileCount = ref<3 | 5 | 10>(5)
const detail = ref<Mode1QuantileDay>()
const loading = ref(false)
const error = ref('')

/** 分位切换后按当前日期重新查询。 */
function changeQuantileCount(value: number | string | boolean): void {
  quantileCount.value = value as 3 | 5 | 10
  void load()
}

/** 分位页签切换：仅改变当前展示的分位表格，不重新查询。 */
function setActiveQuantile(value: string | number): void {
  activeQuantile.value = Number(value)
}

const factorName = computed(() => {
  const previewName = previewStore.results[modeId.value]?.name
  if (previewName) return previewName
  const current = mode1Store.curr
  return current && current.args.base.id === modeId.value ? current.data.name : '因子'
})

function backToPreview(): void {
  void router.push(`/mode1/${modeId.value}`)
}

function toDateString(timestamp: number | null | undefined): string | undefined {
  if (timestamp === null || timestamp === undefined) return undefined
  const date = new Date(timestamp)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

function quantileRows(rows: Mode1DetailRow[]): TableRow[] {
  const items: TableRow[] = rows.map((row, index) => ({
    key: `${row.code}-${index}`,
    isAvg: false,
    code: row.code,
    name: row.name,
    close: row.close,
    changePercent: row.change_percent,
    turnover: row.turnover,
    factor: row.factor,
    raw: row,
  }))

  // 分位末尾：前端计算各项平均值并以特化行展示
  const average = (pick: (row: Mode1DetailRow) => number): number | null => {
    if (rows.length === 0) return null
    const values = rows.map(pick).filter(Number.isFinite)
    if (values.length === 0) return null
    return values.reduce((sum, value) => sum + value, 0) / values.length
  }
  items.push({
    key: '__avg__',
    isAvg: true,
    code: '平均',
    name: `本分位 ${rows.length} 只`,
    close: average((row) => row.close),
    changePercent: average((row) => row.change_percent),
    turnover: average((row) => row.turnover),
    factor: average((row) => row.factor),
    raw: null,
  })
  return items
}

function renderPercent(value: number | null): string {
  return value === null ? '--' : `${value.toFixed(2)}%`
}

function renderTurnover(value: number | null): string {
  return value === null ? '--' : `${(value * 100).toFixed(2)}%`
}

function renderFactor(value: number | null): string {
  return value === null ? '--' : value.toFixed(4)
}

const totalRows = computed(() =>
  detail.value ? detail.value.quantiles.reduce((sum, group) => sum + group.length, 0) : 0,
)

/** 当前展示的分位下标（对应 detail.quantiles；切换只改展示，不重新查询）。 */
const activeQuantile = ref(0)

/** 分位 Tabs 数据源：name=分位下标，下方仅渲染活动分位的一份表格。 */
const quantileTabs = computed(() =>
  (detail.value?.quantiles ?? []).map((group, index) => ({
    name: index,
    label: `分位 ${index + 1}${group.length ? `（${group.length} 只）` : '（空）'}`,
  })),
)

/**
 * 分位表格行缓存：仅在 detail 更新时重建，展开/收起等交互不再触发 ~1000 行的重建，
 * UiTable 收到的 :data 引用保持稳定，只 patch 被点击的那一行。
 */
const quantileData = ref<TableRow[][]>([])

function rebuildQuantiles(): void {
  quantileData.value = (detail.value?.quantiles ?? []).map((group) => quantileRows(group))
}

/** 列定义与分位无关且不依赖响应式状态：模块级构建一次，避免每次渲染新建列数组触发整表更新。 */
/**
 * 全列显式宽度——本地 UiTable 内建 table-layout:fixed 与 colgroup 列宽，
 * 列宽由声明决定，展开行内容不再参与 auto 布局的列宽分配，避免展开/收起时列宽跳动。
 */
const tableColumns: UiTableColumn<TableRow>[] = [
  { title: '#', key: 'rank', width: 56, render: (row, index) => (row.isAvg ? '' : String(index + 1)) },
  { title: '代码', key: 'code', width: 100 },
  { title: '名称', key: 'name', width: 160 },
  { title: '收盘价', key: 'close', align: 'right', width: 150, render: (row) => (row.close === null ? '--' : row.close.toFixed(2)) },
  { title: '涨跌幅', key: 'changePercent', align: 'right', width: 110, render: (row) => renderPercent(row.changePercent) },
  { title: '换手率', key: 'turnover', align: 'right', width: 110, render: (row) => renderTurnover(row.turnover) },
  { title: '因子值', key: 'factor', align: 'right', width: 170, render: (row) => renderFactor(row.factor) },
  { type: 'expand', width: 48, renderExpand },
]

/** 金额格式化：亿/万（单位：元）。 */
function formatMoney(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return '--'
  const abs = Math.abs(value)
  if (abs >= 1e8) return `${(value / 1e8).toFixed(2)} 亿`
  if (abs >= 1e4) return `${(value / 1e4).toFixed(2)} 万`
  return value.toFixed(2)
}

/** 股本格式化：亿股（单位：股）。 */
function formatShares(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return '--'
  return `${(value / 1e8).toFixed(2)} 亿股`
}

/** 展开内容：完整行情 + 财务。 */
function renderExpand(row: TableRow) {
  const source = row.raw
  if (!source) return ''
  const pct = (value: number | null | undefined) =>
    value === null || value === undefined || !Number.isFinite(value) ? '--' : `${value.toFixed(2)}%`
  // 行情 / 财务各字段组独立成行显示（块级换行，避免挤成一长行）
  return h('div', { class: 'expand-row' }, [
    h('div', { class: 'expand-line' }, `日期 ${source.datetime}｜开 ${source.open.toFixed(2)}｜高 ${source.high.toFixed(2)}｜低 ${source.low.toFixed(2)}｜收 ${source.close.toFixed(2)}`),
    h('div', { class: 'expand-line' }, `涨跌幅 ${pct(source.change_percent)}｜成交量 ${formatMoney(source.volume)}｜成交额 ${formatMoney(source.amount)}｜换手率 ${pct(source.turnover * 100)}｜ST ${source.is_st ? '是' : '否'}`),
    h('div', { class: 'expand-line' }, `总市值 ${formatMoney(source.finance?.total_market)}｜流通市值 ${formatMoney(source.finance?.float_market)}｜总股本 ${formatShares(source.finance?.total_shares)}｜流通股本 ${formatShares(source.finance?.float_shares)}`),
    h('div', { class: 'expand-line' }, `股息率 ${pct(source.finance?.dividend_yield)}｜净利润同比 ${pct(source.finance?.du_profit_rate)}｜归母净利润同比 ${pct(source.finance?.inc_net_profit_rate)}`),
  ])
}

/** UiTable row-key 取值。 */
function rowKeyOf(row: unknown): string {
  return (row as TableRow).key
}

const rowProps = (row: unknown): { style: Record<string, string>; onClick: () => void } => {
  const item = row as TableRow
  return {
    style: item.isAvg
      ? {
          background: 'var(--ui-bg-header, #fafafa)',
          fontWeight: '600',
          color: 'var(--ui-text-regular, #606266)',
        }
      : { cursor: 'pointer' },
    onClick: () => {
      if (item.isAvg) return
      const key = item.key
      expandedKeys.value = expandedKeys.value.includes(key)
        ? expandedKeys.value.filter((expanded) => expanded !== key)
        : [...expandedKeys.value, key]
    },
  }
}

async function load(): Promise<void> {
  if (!request.value) return
  loading.value = true
  error.value = ''
  try {
    const params: ModeRequest & { date?: string } = { ...request.value }
    params.base.count = quantileCount.value
    const date = toDateString(day.value)
    if (date) params.date = date
    else delete params.date
    detail.value = await fetchMode1Detail(params)
    rebuildQuantiles()
    expandedKeys.value = []
    activeQuantile.value = 0
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    detail.value = undefined
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  // 日历边界(独立于因子加载,失败不限界)
  fetchDataRange()
    .then((range) => {
      const minMs = new Date(`${range.min_date}T00:00:00`).getTime()
      const maxMs = new Date(`${range.max_date}T00:00:00`).getTime()
      if (Number.isFinite(minMs)) rangeMinMs.value = minMs
      if (Number.isFinite(maxMs)) rangeMaxMs.value = maxMs
    })
    .catch(() => undefined)
  // 解析因子请求参数：优先用预览保存的参数，其次用当前选中因子条目。
  const cached = loadPreviewParams(modeId.value)
  const current = mode1Store.curr
  request.value = cached ?? (current && current.args.base.id === modeId.value ? current.args : undefined)
  if (!request.value) {
    error.value = '缺少因子请求参数，请先在因子预览页运行一次分析后再进入明细'
    return
  }
  const savedCount = request.value.base.count
  if (savedCount === 3 || savedCount === 5 || savedCount === 10) quantileCount.value = savedCount
  const queryDate = route.query.date
  if (typeof queryDate === 'string' && queryDate) {
    const parsed = new Date(`${queryDate}T00:00:00`)
    if (!Number.isNaN(parsed.getTime())) day.value = parsed.getTime()
  }
  await load()
})
</script>

<template>
  <div class="detail-layout">
    <PageTitleBar :title="`${factorName || '因子'}·明细`" :show-detail="false" @back="backToPreview" />
    <div class="toolbar">
      <UiRadioGroup
        :value="quantileCount"
        size="small"
        :disabled="loading"
        @update:value="changeQuantileCount"
      >
        <UiRadio :value="3">三分位</UiRadio>
        <UiRadio :value="5">五分位</UiRadio>
        <UiRadio :value="10">十分位</UiRadio>
      </UiRadioGroup>
      <UiDatePicker
        v-model:value="day"
        :min="rangeMinMs"
        :max="rangeMaxMs"
        clearable
        format="yyyy-MM-dd"
        style="width: 180px"
        :disabled="loading"
      />
      <UiButton type="primary" size="small" :loading="loading" @click="load">查询</UiButton>
      <span class="count-tip">共 {{ totalRows }} 只 · {{ detail?.count ?? quantileCount }} 分位</span>
    </div>

    <UiCard v-if="loading" size="small"><UiSpin :show="true">加载中...</UiSpin></UiCard>
    <UiCard v-else-if="error" size="small">
      <div class="error-tip">{{ error }}</div>
    </UiCard>
    <template v-else-if="detail">
      <div v-if="detail.quantiles.every((group) => group.length === 0)" class="empty-block">
        <UiEmpty description="该日期无数据（非交易日或超出数据范围）" />
      </div>
      <template v-else>
        <UiTabs
          type="segment"
          :tabs="quantileTabs"
          :value="activeQuantile"
          @update:value="setActiveQuantile"
        />
        <UiTable
          v-model:expanded-row-keys="expandedKeys"
          size="small"
          :columns="tableColumns"
          :data="quantileData[activeQuantile] ?? []"
          :row-key="rowKeyOf"
          :row-props="rowProps"
          :bordered="false"
        />
      </template>
    </template>
  </div>
</template>

<style scoped>
.detail-layout {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 32px;
  max-width: 1440px;
  margin: 0 auto;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* 表格表头滚动吸顶 */
.detail-layout :deep(.ui-table__th) {
  position: sticky;
  top: 0;
  z-index: 10;
  background: var(--ui-bg-header, #fafafa);
}

.count-tip {
  font-size: 12px;
  color: var(--ui-text-secondary, #909399);
}

.error-tip {
  color: var(--ui-color-danger, #d03050);
  padding: 8px 0;
}

.expand-row {
  font-size: 13px;
  color: var(--ui-text-regular, #606266);
  line-height: 1.8;
  padding: 4px 8px;
}

.expand-line {
  white-space: nowrap;
}
</style>
