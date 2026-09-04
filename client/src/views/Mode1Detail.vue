<script setup lang="ts">
import { computed, h, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  NButton,
  NCard,
  NDataTable,
  NDatePicker,
  NRadio,
  NRadioGroup,
  NSpin,
  NTabPane,
  NTabs,
  type DataTableColumns,
} from 'naive-ui'

import PageTitleBar from '@/components/common/PageTitleBar.vue'
import { fetchMode1Detail } from '@/api/mode1'
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
const quantileCount = ref<3 | 5 | 10>(5)
const detail = ref<Mode1QuantileDay>()
const loading = ref(false)
const error = ref('')

/** 分位切换后按当前日期重新查询。 */
function changeQuantileCount(value: number): void {
  quantileCount.value = value as 3 | 5 | 10
  void load()
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

/**
 * 分位表格行缓存：仅在 detail 更新时重建，展开/收起等交互不再触发 ~1000 行的重建，
 * naive-ui 收到的 :data 引用保持稳定，只 patch 被点击的那一行。
 */
const quantileData = ref<TableRow[][]>([])

function rebuildQuantiles(): void {
  quantileData.value = (detail.value?.quantiles ?? []).map((group) => quantileRows(group))
}

/** 列定义与分位无关且不依赖响应式状态：模块级构建一次，避免每次渲染新建列数组触发整表更新。 */
/**
 * 全列显式宽度 + table-layout:fixed（见样式）——列宽由声明决定，
 * 展开行内容不再参与 auto 布局的列宽分配，避免展开/收起时列宽跳动。
 */
const tableColumns: DataTableColumns<TableRow> = [
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
  if (!source) return null
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

const rowProps = (row: TableRow) => ({
  style: row.isAvg ? { background: '#fafafa', fontWeight: 600, color: '#606266' } : { cursor: 'pointer' },
  onClick: () => {
    if (row.isAvg) return
    const key = row.key
    expandedKeys.value = expandedKeys.value.includes(key)
      ? expandedKeys.value.filter((item) => item !== key)
      : [...expandedKeys.value, key]
  },
})

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
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    detail.value = undefined
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
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
      <NRadioGroup
        :value="quantileCount"
        size="small"
        :disabled="loading"
        @update:value="changeQuantileCount"
      >
        <NRadio :value="3">三分位</NRadio>
        <NRadio :value="5">五分位</NRadio>
        <NRadio :value="10">十分位</NRadio>
      </NRadioGroup>
      <NDatePicker
        v-model:value="day"
        type="date"
        clearable
        format="yyyy-MM-dd"
        style="width: 180px"
        :disabled="loading"
      />
      <NButton type="primary" size="small" :loading="loading" @click="load">查询</NButton>
      <span class="count-tip">共 {{ totalRows }} 只 · {{ detail?.count ?? quantileCount }} 分位</span>
    </div>

    <NCard v-if="loading" size="small"><NSpin>加载中...</NSpin></NCard>
    <NCard v-else-if="error" size="small">
      <div class="error-tip">{{ error }}</div>
    </NCard>
    <template v-else-if="detail">
      <div v-if="detail.quantiles.every((group) => group.length === 0)" class="empty-block">
        <NEmpty description="该日期无数据（非交易日或超出数据范围）" />
      </div>
      <NTabs v-else type="segment">
        <NTabPane
          v-for="(group, index) in detail.quantiles"
          :key="index"
          :name="index"
          :tab="`分位 ${index + 1}${group.length ? `（${group.length} 只）` : '（空）'}`"
        >
          <NDataTable
            v-model:expanded-row-keys="expandedKeys"
            size="small"
            :columns="tableColumns"
            :data="quantileData[index] ?? []"
            :row-key="(row: TableRow) => row.key"
            :row-props="rowProps"
            :bordered="false"
          />
        </NTabPane>
      </NTabs>
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


/* 路径 A：解除 naive 滚动容器与 NTabs 内容层包装，让 sticky 相对视口生效 */
.detail-layout :deep(.n-data-table-base-table-body.n-scrollbar),
.detail-layout :deep(.n-scrollbar-container),
.detail-layout :deep(.n-tabs-pane-wrapper) {
  overflow: visible;
}

/* 表格表头滚动吸顶 */
.detail-layout :deep(.n-data-table-thead .n-data-table-th) {
  position: sticky;
  top: 0;
  z-index: 10;
  background: rgb(250, 250, 252);
}

/* 固定表格布局：列宽由声明决定，展开行不再影响各列宽度（naive 内联 auto 需 !important 覆盖） */
.detail-layout :deep(.n-data-table-table) {
  table-layout: fixed !important;
  width: 100%;
}

.count-tip {
  font-size: 12px;
  color: #909399;
}

.error-tip {
  color: #d03050;
  padding: 8px 0;
}


.expand-row {
  font-size: 13px;
  color: #606266;
  line-height: 1.8;
  padding: 4px 8px;
}

.expand-line {
  white-space: nowrap;
}
</style>
