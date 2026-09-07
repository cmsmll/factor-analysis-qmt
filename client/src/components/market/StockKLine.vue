<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NConfigProvider, NDatePicker, dateZhCN, zhCN } from 'naive-ui'
import {
  CandlestickSeries,
  ColorType,
  CrosshairMode,
  HistogramSeries,
  LineSeries,
  createChart,
  type CandlestickData,
  type HistogramData,
  type IChartApi,
  type ISeriesApi,
  type LineData,
  type MouseEventHandler,
  type Time,
} from 'lightweight-charts'

import { fetchMarketKline } from '@/api/market'
import type { MarketKline, MarketKlineRow } from '@/api/market'
import {
  chartTimeToDate,
  formatDateTime,
  formatLargeNumber,
  formatPercent,
  formatPlainPercent,
  formatPrice,
  toChartDate,
} from '@/utils/klineTools'

defineOptions({ name: 'StockKLine' })

const route = useRoute()
const router = useRouter()

const props = withDefaults(
  defineProps<{
    code: string
    /** 行情日期 YYYY-MM-DD：加载后以该日为中心显示前/后约半年（缺省全量 fitContent） */
    centerDate?: string
  }>(),
  {
    code: '',
    centerDate: '',
  },
)

interface DisplayMarketItem {
  date: string
  open: number
  close: number
  high: number
  low: number
  volume: number
  turnover: number
  turnoverRate: number
  changePercent: number
  dayAmplitude: number
}

const UP_COLOR = '#ef4444'
const DOWN_COLOR = '#22c55e'
const UP_VOLUME_COLOR = 'rgba(239, 68, 68, 0.45)'
const DOWN_VOLUME_COLOR = 'rgba(34, 197, 94, 0.45)'

const chartContainer = ref<HTMLDivElement | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)
const stockName = ref('')
const stockCode = ref('')
/** 交易所后缀（SH/SZ/BJ），由接口 exchange 推导 */
const stockSuffix = ref('')
const currentItem = ref<DisplayMarketItem | null>(null)
/** 全量数据行缓存（供按日期定位窗口） */
let dataRows: MarketKlineRow[] = []
const latestItem = ref<DisplayMarketItem | null>(null)
/** 最后一次悬停的日期（鼠标离开时保持展示该根，不回退到最新根）。 */
const lastHoveredDate = ref<string | null>(null)
const showCandles = ref(true)
const showMa5 = ref(false)
const showMa10 = ref(false)
const showMa20 = ref(false)
const showMa30 = ref(false)
const showMa60 = ref(false)
const showVolume = ref(true)

let chart: IChartApi | null = null
let candleSeries: ISeriesApi<'Candlestick'> | null = null
let ma5Series: ISeriesApi<'Line'> | null = null
let ma10Series: ISeriesApi<'Line'> | null = null
let ma20Series: ISeriesApi<'Line'> | null = null
let ma30Series: ISeriesApi<'Line'> | null = null
let ma60Series: ISeriesApi<'Line'> | null = null
let volumeSeries: ISeriesApi<'Histogram'> | null = null
let resizeObserver: ResizeObserver | null = null
let crosshairHandler: MouseEventHandler<Time> | null = null
let rowByDate = new Map<string, MarketKlineRow>()
/** 日期文本 -> dataRows 下标（方向键逐根导航用）。 */
let idxByDate = new Map<string, number>()
let loadSeq = 0

const priceClass = computed(() => {
  const item = currentItem.value
  if (!item) return ''
  return item.close >= item.open ? 'up' : 'down'
})

function toDisplayItem(row: MarketKlineRow): DisplayMarketItem {
  return {
    date: formatDateTime(row.datetime),
    open: row.open,
    close: row.close,
    high: row.high,
    low: row.low,
    volume: Math.round(row.volume),
    turnover: row.turnover,
    turnoverRate: row.turnover_rate,
    changePercent: row.change_percent,
    dayAmplitude: row.low === 0 ? 0 : (row.high - row.low) / row.low,
  }
}

function assertMarketData(value: unknown): asserts value is MarketKline {
  if (!value || typeof value !== 'object') {
    throw new Error('行情数据格式错误：根节点不是对象')
  }

  const maybeData = value as Partial<MarketKline>
  if (!Array.isArray(maybeData.market_data)) {
    throw new Error('行情数据格式错误：market_data 必须是数组')
  }

  if (maybeData.market_data.length === 0) {
    throw new Error('接口未返回行情数据')
  }
}

function buildCandles(rows: MarketKlineRow[]): CandlestickData<Time>[] {
  return rows.map((row) => ({
    time: toChartDate(row.datetime),
    open: row.open,
    high: row.high,
    low: row.low,
    close: row.close,
  }))
}

function buildVolumes(rows: MarketKlineRow[]): HistogramData<Time>[] {
  return rows.map((row) => ({
    time: toChartDate(row.datetime),
    value: Math.round(row.volume),
    color: row.close >= row.open ? UP_VOLUME_COLOR : DOWN_VOLUME_COLOR,
  }))
}

function calculateSma(rows: MarketKlineRow[], period: number): LineData<Time>[] {
  const result: LineData<Time>[] = []
  let sum = 0

  rows.forEach((row, index) => {
    sum += row.close

    if (index >= period) {
      const previous = rows[index - period]
      if (previous) {
        sum -= previous.close
      }
    }

    if (index >= period - 1) {
      result.push({
        time: toChartDate(row.datetime),
        value: Number((sum / period).toFixed(2)),
      })
    }
  })

  return result
}

/** 交易所名称 -> 代码后缀（上交所.SH / 深交所.SZ / 北交所.BJ）。 */
function exchangeSuffix(exchange?: string): string {
  const name = exchange ?? ''
  if (name.includes('上交') || name.includes('上海') || name.includes('SH')) return '.SH'
  if (name.includes('深交') || name.includes('深圳') || name.includes('SZ')) return '.SZ'
  if (name.includes('北交') || name.includes('北京') || name.includes('BJ')) return '.BJ'
  return ''
}

/** 'YYYY-MM-DD' -> 本地零点毫秒。 */
function parseDateText(text: string): number {
  const parsed = new Date(`${text}T00:00:00`)
  return Number.isNaN(parsed.getTime()) ? 0 : parsed.getTime()
}

/** 毫秒 -> 'YYYY-MM-DD'。 */
function toDateText(timestamp: number): string {
  const date = new Date(timestamp)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** 当前选中日期的毫秒值（供日期选择器）。 */
const pickerValue = computed<number | null>(() => {
  const date = currentItem.value?.date
  if (!date) return null
  const value = parseDateText(date)
  return value > 0 ? value : null
})

/** 选择日期：定位窗口 ±半年、联动当前/8格、同步 URL query.date。 */
function applySelectedDate(text: string): void {
  if (dataRows.length === 0) return
  const target = dataRows.findIndex((row) => toChartDate(row.datetime) === text)
  if (target >= 0 && chart) {
    const halfYear = 125
    chart.timeScale().setVisibleLogicalRange({
      from: Math.max(0, target - halfYear),
      to: Math.min(dataRows.length - 1, target + halfYear),
    })
  }
  const row = rowByDate.get(text)
  if (row) {
    lastHoveredDate.value = text
    currentItem.value = toDisplayItem(row)
  }
  void router.replace({ query: { date: text } })
}

function onDatePick(value: number | null): void {
  if (value === null) return
  const text = toDateText(value)
  if (text) applySelectedDate(text)
}

function setCurrentByDate(date: string | null): void {
  if (!date) {
    // 鼠标离开 canvas：保持最后一次悬停的 K 线；从未悬停则显示最新一根
    const fallback = lastHoveredDate.value
    if (fallback) {
      const row = rowByDate.get(fallback)
      currentItem.value = row ? toDisplayItem(row) : latestItem.value
    } else {
      currentItem.value = latestItem.value
    }
    return
  }

  const row = rowByDate.get(date)
  if (row) {
    lastHoveredDate.value = date
    currentItem.value = toDisplayItem(row)
  } else {
    currentItem.value = latestItem.value
  }
}

/** 方向键逐根导航：当前根取最近悬停/选中日，无则从最新一根开始。 */
function stepBar(direction: 1 | -1): void {
  if (dataRows.length === 0 || !chart || !candleSeries) return
  const active = lastHoveredDate.value
  const anchor = active && idxByDate.has(active) ? (idxByDate.get(active) as number) : dataRows.length - 1
  const target = anchor + direction
  if (target < 0 || target >= dataRows.length) return

  const row = dataRows[target]
  if (!row) return

  const date = toChartDate(row.datetime)

  // 顶栏/8格/日期选择器联动（与悬停同一状态源）
  setCurrentByDate(date)
  // 十字线吸附到该根（隐藏/透明系列仍在，取收盘价）
  chart.setCrosshairPosition(row.close, date, candleSeries)

  // 目标根临近可视边缘时平移窗口（保持缩放宽度），整段可见则不滚
  const range = chart.timeScale().getVisibleLogicalRange()
  if (range && range.from < range.to) {
    const width = range.to - range.from
    const lastIndex = dataRows.length - 1
    if (width < lastIndex + 1) {
      const margin = Math.min(8, Math.max(2, Math.round(width * 0.06)))
      let newFrom: number | null = null
      if (target < range.from + margin) {
        newFrom = Math.max(0, target - margin)
      } else if (target > range.to - margin) {
        newFrom = Math.min(Math.max(0, lastIndex - width), target - width + margin)
      }
      if (newFrom !== null) {
        chart.timeScale().setVisibleLogicalRange({ from: newFrom, to: newFrom + width })
      }
    }
  }
}

/** 键盘监听：←/→ 逐根移动；编辑态/日历弹层内不拦截。 */
function handleBarKeydown(event: KeyboardEvent): void {
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
  const target = event.target as HTMLElement | null
  if (
    target &&
    (target.tagName === 'INPUT' ||
      target.tagName === 'TEXTAREA' ||
      target.isContentEditable ||
      Boolean(target.closest('.n-input, .n-date-panel')))
  ) {
    return
  }
  event.preventDefault()
  stepBar(event.key === 'ArrowRight' ? 1 : -1)
}

function applySeriesVisibility(): void {
  candleSeries?.applyOptions({ visible: showCandles.value })
  ma5Series?.applyOptions({ visible: showMa5.value })
  ma10Series?.applyOptions({ visible: showMa10.value })
  ma20Series?.applyOptions({ visible: showMa20.value })
  ma30Series?.applyOptions({ visible: showMa30.value })
  ma60Series?.applyOptions({ visible: showMa60.value })
  volumeSeries?.applyOptions({ visible: showVolume.value })
}

function toggleSeries(
  type: 'candles' | 'ma5' | 'ma10' | 'ma20' | 'ma30' | 'ma60' | 'volume',
): void {
  if (type === 'candles') {
    showCandles.value = !showCandles.value
  } else if (type === 'ma5') {
    showMa5.value = !showMa5.value
  } else if (type === 'ma10') {
    showMa10.value = !showMa10.value
  } else if (type === 'ma20') {
    showMa20.value = !showMa20.value
  } else if (type === 'ma30') {
    showMa30.value = !showMa30.value
  } else if (type === 'ma60') {
    showMa60.value = !showMa60.value
  } else {
    showVolume.value = !showVolume.value
  }

  applySeriesVisibility()
}

function createKLineChart(
  container: HTMLDivElement,
  rows: MarketKlineRow[],
): void {
  const datetimeByDate = new Map(
    rows.map((row) => [toChartDate(row.datetime), row.datetime]),
  )
  const formatCrosshairTime = (time: Time): string => {
    const date = chartTimeToDate(time)
    const datetime = date ? datetimeByDate.get(date) : undefined
    return datetime ? formatDateTime(datetime) : date || ''
  }

  const formatTimeTick = (time: Time, tickMarkType: number): string => {
    const date = chartTimeToDate(time)
    if (!date) return ''

    const [year, month, day] = date.split('-')
    if (!year || !month || !day) return date

    if (tickMarkType === 0) return `${year}年`
     
    if (tickMarkType === 1) return `${Number(month)}月`
    return ''
  }

  chart = createChart(container, {
    autoSize: true,
    layout: {
      attributionLogo: false,
      background: { type: ColorType.Solid, color: '#080d16' },
      fontSize: 15,
      textColor: '#94a3b8',
    },
    grid: {
      vertLines: { color: 'rgba(30, 41, 59, 0.72)' },
      horzLines: { color: 'rgba(30, 41, 59, 0.72)' },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
      horzLine: {
        labelVisible: true,
      },
    },
    rightPriceScale: {
      borderColor: 'rgba(148, 163, 184, 0.22)',
      scaleMargins: {
        top: 0.08,
        bottom: 0.24,
      },
    },
    timeScale: {
      borderColor: 'rgba(148, 163, 184, 0.22)',
      secondsVisible: false,
      tickMarkFormatter: formatTimeTick,
      timeVisible: true,
    },
    localization: {
      priceFormatter: (price: number) => price.toFixed(2),
      timeFormatter: formatCrosshairTime,
    },
  })

  candleSeries = chart.addSeries(CandlestickSeries, {
    upColor: UP_COLOR,
    downColor: DOWN_COLOR,
    borderUpColor: UP_COLOR,
    borderDownColor: DOWN_COLOR,
    wickUpColor: UP_COLOR,
    wickDownColor: DOWN_COLOR,
    priceLineVisible: false,
    lastValueVisible: false,
  })

  ma5Series = chart.addSeries(LineSeries, {
    color: '#facc15',
    lineWidth: 2,
    priceLineVisible: false,
    lastValueVisible: false,
  })

  ma10Series = chart.addSeries(LineSeries, {
    color: '#38bdf8',
    lineWidth: 2,
    priceLineVisible: false,
    lastValueVisible: false,
  })

  ma20Series = chart.addSeries(LineSeries, {
    color: '#a78bfa',
    lineWidth: 2,
    priceLineVisible: false,
    lastValueVisible: false,
  })

  ma30Series = chart.addSeries(LineSeries, {
    color: '#fb923c',
    lineWidth: 2,
    priceLineVisible: false,
    lastValueVisible: false,
  })

  ma60Series = chart.addSeries(LineSeries, {
    color: '#f472b6',
    lineWidth: 2,
    priceLineVisible: false,
    lastValueVisible: false,
  })

  volumeSeries = chart.addSeries(HistogramSeries, {
    priceFormat: {
      type: 'volume',
    },
    priceScaleId: 'volume',
    priceLineVisible: false,
    lastValueVisible: false,
  })

  chart.priceScale('volume').applyOptions({
    scaleMargins: {
      top: 0.78,
      bottom: 0,
    },
    borderVisible: false,
  })

  candleSeries.setData(buildCandles(rows))
  ma5Series.setData(calculateSma(rows, 5))
  ma10Series.setData(calculateSma(rows, 10))
  ma20Series.setData(calculateSma(rows, 20))
  ma30Series.setData(calculateSma(rows, 30))
  ma60Series.setData(calculateSma(rows, 60))
  volumeSeries.setData(buildVolumes(rows))
  applySeriesVisibility()

  crosshairHandler = (param) => {
    setCurrentByDate(chartTimeToDate(param.time))
  }

  chart.subscribeCrosshairMove(crosshairHandler)
  chart.timeScale().fitContent()
}

function disposeChart(): void {
  resizeObserver?.disconnect()
  resizeObserver = null

  if (chart && crosshairHandler) {
    chart.unsubscribeCrosshairMove(crosshairHandler)
  }

  chart?.remove()
  chart = null
  candleSeries = null
  ma5Series = null
  ma10Series = null
  ma20Series = null
  ma30Series = null
  ma60Series = null
  volumeSeries = null
  crosshairHandler = null
}

async function loadMarketData(): Promise<void> {
  const seq = ++loadSeq
  loading.value = true
  error.value = null
  disposeChart()

  try {
    const json = await fetchMarketKline(props.code)
    if (seq !== loadSeq) return

    assertMarketData(json)

    stockName.value = json.nam || '未知股票'
    stockCode.value = json.code
    stockSuffix.value = exchangeSuffix(json.exchange)

    const rows = json.market_data
    dataRows = rows
    const last = rows.at(-1)
    if (!last) {
      throw new Error('没有可渲染的行情数据')
    }

    rowByDate = new Map(rows.map((row) => [toChartDate(row.datetime), row]))
    idxByDate = new Map(rows.map((row, index) => [toChartDate(row.datetime), index]))
    latestItem.value = toDisplayItem(last)
    currentItem.value = latestItem.value
    lastHoveredDate.value = null

    await nextTick()

    const container = chartContainer.value
    if (!container) {
      throw new Error('图表容器未渲染')
    }

    createKLineChart(container, rows)

    // 指定行情日期：以该根为中心显示前/后约半年（半年≈125 个交易日）；否则全量 fitContent
    const centerTarget = props.centerDate
      ? rows.findIndex((row) => toChartDate(row.datetime) === props.centerDate)
      : -1
    if (centerTarget >= 0 && chart) {
      const halfYear = 125
      chart.timeScale().setVisibleLogicalRange({
        from: Math.max(0, centerTarget - halfYear),
        to: Math.min(rows.length - 1, centerTarget + halfYear),
      })
    }

    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (!entry || !chart) return

      chart.applyOptions({
        width: Math.floor(entry.contentRect.width),
        height: Math.floor(entry.contentRect.height),
      })
    })
    resizeObserver.observe(container)
  } catch (err) {
    if (seq === loadSeq) {
      error.value = err instanceof Error ? err.message : '加载行情数据失败'
    }
  } finally {
    if (seq === loadSeq) {
      loading.value = false
    }
  }
}

watch(
  () => props.code,
  () => {
    disposeChart()
    rowByDate = new Map()
    stockName.value = ''
    stockCode.value = props.code
    stockSuffix.value = ''
    dataRows = []
    latestItem.value = null
    currentItem.value = null
    lastHoveredDate.value = null
    void loadMarketData()
  },
)

onMounted(() => {
  window.addEventListener('keydown', handleBarKeydown)
  void loadMarketData()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleBarKeydown)
  disposeChart()
})
</script>

<template>
  <main class="stock-page">
    <section class="stock-panel">
      <header class="stock-header">
        <!-- 左：名称（字号调小）+ 代码 -->
        <div class="stock-title-block">
          <div class="stock-name">
            <span class="stock-name-text">{{ stockName || '股票行情' }}</span>
            <span v-if="stockCode" class="stock-name-code">{{ stockCode }}{{ stockSuffix }}</span>
          </div>
        </div>

        <!-- 中：日期，点击直接展开日历（单步） -->
        <NConfigProvider :locale="zhCN" :date-locale="dateZhCN">
          <NDatePicker
            class="header-date-picker"
            :value="pickerValue"
            type="date"
            size="small"
            :clearable="false"
            :placeholder="'—'"
            format="yyyy-MM-dd"
            to=".stock-page"
            @update:value="onDatePick"
          />
        </NConfigProvider>

        <!-- 右：价格与涨跌幅 -->
        <div v-if="currentItem" class="price-row price-cell">
          <strong :class="priceClass">{{
            formatPrice(currentItem.close)
          }}</strong>
          <span :class="priceClass">{{
            formatPercent(currentItem.changePercent)
          }}</span>
        </div>
      </header>

      <div class="quote-strip" :class="{ muted: !currentItem }">
        <template v-if="currentItem">
          <div>
            <span>开盘</span>
            <strong>{{ formatPrice(currentItem.open) }}</strong>
          </div>
          <div>
            <span>收盘</span>
            <strong>{{ formatPrice(currentItem.close) }}</strong>
          </div>
          <div>
            <span>最高</span>
            <strong>{{ formatPrice(currentItem.high) }}</strong>
          </div>
          <div>
            <span>最低</span>
            <strong>{{ formatPrice(currentItem.low) }}</strong>
          </div>
          <div>
            <span>成交量</span>
            <strong>{{ formatLargeNumber(currentItem.volume) }}</strong>
          </div>
          <div>
            <span>成交额</span>
            <strong>{{ formatLargeNumber(currentItem.turnover) }}</strong>
          </div>
          <div>
            <span>换手率</span>
            <strong>{{ formatPlainPercent(currentItem.turnoverRate) }}</strong>
          </div>
          <div>
            <span>日振幅</span>
            <strong>{{ formatPlainPercent(currentItem.dayAmplitude) }}</strong>
          </div>
        </template>
        <span v-else>等待行情数据...</span>
      </div>

      <div class="chart-shell">
        <div class="chart-toolbar">
          <div class="legend-group">
            <button
              type="button"
              class="legend candle"
              :class="{ active: showCandles }"
              @click="toggleSeries('candles')"
            >
              K线
            </button>
            <button
              type="button"
              class="legend volume"
              :class="{ active: showVolume }"
              @click="toggleSeries('volume')"
            >
              成交量
            </button>
            <button
              type="button"
              class="legend ma5"
              :class="{ active: showMa5 }"
              @click="toggleSeries('ma5')"
            >
              MA5
            </button>
            <button
              type="button"
              class="legend ma10"
              :class="{ active: showMa10 }"
              @click="toggleSeries('ma10')"
            >
              MA10
            </button>
            <button
              type="button"
              class="legend ma20"
              :class="{ active: showMa20 }"
              @click="toggleSeries('ma20')"
            >
              MA20
            </button>
            <button
              type="button"
              class="legend ma30"
              :class="{ active: showMa30 }"
              @click="toggleSeries('ma30')"
            >
              MA30
            </button>
            <button
              type="button"
              class="legend ma60"
              :class="{ active: showMa60 }"
              @click="toggleSeries('ma60')"
            >
              MA60
            </button>
          </div>
        </div>

        <div class="chart-area">
          <div ref="chartContainer" class="chart-container"></div>

          <div v-if="loading" class="state-overlay">
            <span class="loader"></span>
            <p>加载行情数据中...</p>
          </div>

          <div v-else-if="error" class="state-overlay error-state">
            <strong>渲染失败</strong>
            <p>{{ error }}</p>
          </div>
        </div>
      </div>
    </section>
  </main>
</template>

<style scoped>
.stock-page {
  box-sizing: border-box;
  height: 100vh;
  min-height: 0;
  overflow: hidden;
  padding: 18px;
  color: #e2e8f0;
  background:
    radial-gradient(circle at 12% 0%, rgba(239, 68, 68, 0.16), transparent 28%),
    radial-gradient(
      circle at 88% 12%,
      rgba(56, 189, 248, 0.12),
      transparent 30%
    ),
    #020617;
}

.stock-panel {
  box-sizing: border-box;
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  flex-direction: column;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 18px;
  background: rgba(15, 23, 42, 0.94);
  box-shadow: 0 24px 80px rgba(2, 6, 23, 0.42);
}

.stock-header {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 12px;
  padding: 12px 22px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.14);
}

/* 左格：名称 + 代码 */
.stock-title-block {
  min-width: 0;
  justify-self: start;
}

.stock-name {
  display: inline-flex;
  align-items: baseline;
  min-width: 0;
  overflow: hidden;
  color: #f8fafc;
  font-size: 20px;
  font-weight: 700;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stock-name-code {
  margin-left: 8px;
  color: #64748b;
  font-size: 20px;
  font-weight: 500;
  letter-spacing: 0.5px;
}

/* 中格：无边框文本型日期选择器（点击一步展开日历） */
.header-date-picker {
  width: 160px;
}

/* naive 用 .n-input__border/.n-input__state-border 按 --n-border 画边框：
   直接覆盖其主题变量（inline 定义，需 !important），并把输入底色置透明 */
.stock-page :deep(.header-date-picker .n-input),
.stock-page :deep(.header-date-picker .n-input:hover),
.stock-page :deep(.header-date-picker .n-input.n-input--focus),
.stock-page :deep(.header-date-picker .n-input.n-input--active) {
  --n-border: none !important;
  --n-border-hover: none !important;
  --n-border-focus: none !important;
  --n-border-disabled: none !important;
  --n-box-shadow-focus: none !important;
  --n-color: transparent !important;
  --n-color-focus: transparent !important;
  border: none !important;
  border-color: transparent !important;
  background: transparent !important;
  box-shadow: none !important;
}

.stock-page :deep(.header-date-picker .n-input__input-el),
.stock-page :deep(.header-date-picker .n-input__input) {
  color: #e2e8f0;
  font-size: 20px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.stock-page :deep(.header-date-picker .n-input__suffix) {
  display: none;
}

/* 右格：价格与涨跌幅 */
.price-cell {
  justify-self: end;
}

.price-row {
  display: flex;
  align-items: baseline;
  gap: 16px;
}

.price-row strong,
.price-row span {
  font-size: 20px;
  font-weight: 800;
  line-height: 1;
}

.up {
  color: #f87171;
}

.down {
  color: #4ade80;
}

.quote-strip {
  display: grid;
  grid-template-columns: repeat(8, minmax(0, 1fr));
  border-bottom: 1px solid rgba(148, 163, 184, 0.14);
  background: rgba(2, 6, 23, 0.28);
}

.quote-strip > div {
  display: flex;
  min-width: 0;
  align-items: baseline;
  gap: 6px;
  padding: 7px 0 7px 14px;
  border-right: 1px solid rgba(148, 163, 184, 0.1);
}

.quote-strip > div:last-child {
  border-right: 0;
}

.quote-strip span {
  flex: 0 0 auto;
  overflow: hidden;
  color: #94a3b8;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quote-strip strong {
  display: block;
  min-width: 0;
  overflow: hidden;
  color: #f8fafc;
  font-size: 15px;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quote-strip strong.up {
  color: #f87171;
}

.quote-strip strong.down {
  color: #4ade80;
}

.quote-strip.muted {
  padding: 14px;
  color: #94a3b8;
}

.chart-shell {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}

.chart-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 10px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.12);
}

.legend-group {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.legend {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 12px;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 14px;
  background: rgba(15, 23, 42, 0.66);
  color: #64748b;
  cursor: pointer;
  font-size: 13px;
}

.legend.active {
  border-color: rgba(148, 163, 184, 0.34);
  color: #cbd5e1;
}

.legend::before {
  width: 12px;
  height: 2px;
  border-radius: 999px;
  opacity: 0.45;
  content: '';
}

.legend.active::before {
  opacity: 1;
}

.legend.candle::before {
  background: linear-gradient(90deg, #ef4444 50%, #22c55e 50%);
}

.legend.ma5::before {
  background: #facc15;
}

.legend.ma10::before {
  background: #38bdf8;
}

.legend.ma20::before {
  background: #a78bfa;
}

.legend.ma30::before {
  background: #fb923c;
}

.legend.ma60::before {
  background: #f472b6;
}

.legend.volume::before {
  background: linear-gradient(90deg, #ef4444 50%, #22c55e 50%);
}

.chart-area {
  position: relative;
  min-height: 0;
  flex: 1;
  overflow: hidden;
}

.chart-container {
  position: absolute;
  inset: 0;
}

.state-overlay {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 10px;
  background: rgba(2, 6, 23, 0.72);
  color: #cbd5e1;
  text-align: center;
}

.loader {
  width: 28px;
  height: 28px;
  border: 3px solid rgba(148, 163, 184, 0.3);
  border-top-color: #38bdf8;
  border-radius: 999px;
  animation: spin 0.8s linear infinite;
}

.error-state {
  color: #fecaca;
}

.error-state strong {
  color: #f87171;
  font-size: 18px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 1120px) {
  .quote-strip {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .stock-page {
    padding: 10px;
  }

  .stock-panel {
    border-radius: 12px;
  }

  .stock-header {
    grid-template-columns: 1fr auto;
    gap: 8px;
  }

  .stock-title-block {
    grid-column: 1;
  }

  .header-date-picker {
    grid-column: 1 / -1;
    grid-row: 2;
    justify-self: start;
    width: 150px;
  }

  .price-cell {
    grid-column: 2;
    grid-row: 1;
  }

  .price-row strong,
  .price-row span {
    font-size: 20px;
  }

  .chart-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .quote-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
