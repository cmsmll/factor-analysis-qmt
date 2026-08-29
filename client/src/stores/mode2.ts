import { reactive, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import { fetchMode2History, fetchMode2Select } from '@/api/mode2'
import type { ModeFilter } from '@/types/mode1'
import type {
  HistoryParams,
  Mode2History,
  Mode2Stage,
  SelectParams,
  StockItem,
} from '@/types/mode2'

const FILTER_CACHE_KEY = 'mode2-filter'

/** 微盘股预设：市值最小 400 只 → 其中收盘价最低 80 只。 */
export const MICROCAP_STAGES: Mode2Stage[] = [
  { field: 'TotalMarket', direction: 'Asc', filter: 'None', select: 400 },
  { field: 'Close', direction: 'Asc', filter: 'None', select: 80 },
]

function cloneBase(base: ModeFilter): ModeFilter {
  return { ...base, sector: [...base.sector], indice: [...base.indice] }
}

export const useMode2Store = defineStore('mode2', () => {
  // 股票池由看板固定过滤区驱动（applyPool），不含参数化选股配置。
  const base = reactive<ModeFilter>({
    start: '',
    end: '',
    filter_bz: false,
    filter_st: false,
    sector: [],
    indice: [],
  })
  const currentDate = ref('')

  // 结果与加载状态
  const history = shallowRef<Mode2History | null>(null)
  const stockItems = shallowRef<StockItem[]>([])
  const historyLoading = ref(false)
  const selectLoading = ref(false)
  const historyError = ref('')
  const selectError = ref('')

  let historyRequestVersion = 0
  let selectRequestVersion = 0
  // 名单缓存：键 = 完整请求（含股票池与日期），池变更后自动失效
  const selectCache = new Map<string, StockItem[]>()

  function buildSelectReq(date: string): SelectParams {
    return {
      stages: MICROCAP_STAGES,
      profit_mode: 1,
      date,
      base: cloneBase(base),
    }
  }

  function buildHistoryReq(): HistoryParams {
    return {
      stages: MICROCAP_STAGES,
      profit_mode: 1,
      base: cloneBase(base),
    }
  }

  /** 由看板固定过滤区驱动：更新股票池并刷新回测与当前日期名单。 */
  async function applyPool(pool: {
    start: string
    end: string
    sector: string[]
    indice: string[]
  }): Promise<void> {
    base.start = pool.start
    base.end = pool.end
    base.sector = [...pool.sector]
    base.indice = [...pool.indice]
    try {
      localStorage.setItem(FILTER_CACHE_KEY, JSON.stringify(cloneBase(base)))
    } catch {
      // localStorage full or unavailable
    }
    await loadHistory()
  }

  /** 区间回测；股票池变更后同步刷新当前日期名单。 */
  async function loadHistory(): Promise<void> {
    if (!base.start || !base.end) return
    const version = ++historyRequestVersion
    historyLoading.value = true
    historyError.value = ''
    try {
      const data = await fetchMode2History(buildHistoryReq())
      if (historyRequestVersion !== version) return
      history.value = data
      const dates = uniqueDates(data.datetime)
      const last = dates.at(-1) ?? ''
      if (!currentDate.value || !dates.includes(currentDate.value)) {
        currentDate.value = last
      }
      await loadSelect(currentDate.value)
    } catch (error) {
      if (historyRequestVersion !== version) return
      historyError.value = error instanceof Error ? error.message : '获取回测数据失败'
    } finally {
      if (historyRequestVersion === version) historyLoading.value = false
    }
  }

  /** 单日选股名单（按完整请求缓存）。 */
  async function loadSelect(date: string): Promise<void> {
    if (!date || !base.start) return
    const req = buildSelectReq(date)
    const key = JSON.stringify(req)
    const cached = selectCache.get(key)
    if (cached) {
      stockItems.value = cached
      return
    }
    const version = ++selectRequestVersion
    selectLoading.value = true
    selectError.value = ''
    try {
      const items = await fetchMode2Select(req)
      if (selectRequestVersion !== version) return
      stockItems.value = items
      selectCache.set(key, items)
    } catch (error) {
      if (selectRequestVersion !== version) return
      selectError.value = error instanceof Error ? error.message : '获取选股名单失败'
    } finally {
      if (selectRequestVersion === version) selectLoading.value = false
    }
  }

  return {
    base,
    currentDate,
    history,
    stockItems,
    historyLoading,
    selectLoading,
    historyError,
    selectError,
    applyPool,
    loadHistory,
    loadSelect,
  }
})

/** 交易日去重（回测首点为基线点，与首个交易日重复）。 */
export function uniqueDates(datetime: string[]): string[] {
  return [...new Set(datetime)]
}
