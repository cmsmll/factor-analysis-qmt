import { reactive, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import { fetchMode2History, fetchMode2Select } from '@/api/mode2'
import { fetchPeriods } from '@/api/mode1'
import type { ModeFilter, Period } from '@/types/mode1'
import type {
  HistoryParams,
  Mode2Direction,
  Mode2Field,
  Mode2FilterType,
  Mode2History,
  Mode2OpFilter,
  SelectParams,
  StockItem,
} from '@/types/mode2'

const FILTER_CACHE_KEY = 'mode2-filter'

function loadCachedBase(): Partial<ModeFilter> | null {
  try {
    const raw = localStorage.getItem(FILTER_CACHE_KEY)
    if (!raw) return null
    return JSON.parse(raw) as Partial<ModeFilter>
  } catch {
    return null
  }
}

function cloneBase(base: ModeFilter): ModeFilter {
  return { ...base, sector: [...base.sector], indice: [...base.indice] }
}

export const useMode2Store = defineStore('mode2', () => {
  // 选股参数
  const field = ref<Mode2Field>('TotalMarket')
  const direction = ref<Mode2Direction>('Desc')
  const filterType = ref<Mode2FilterType>('None')
  const threshold = ref<number | null>(null)
  const selectN = ref<number | null>(10)
  const profitMode = ref<1 | 2 | 3 | 4>(1)
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
  const periods = shallowRef<Period[]>([])
  const history = shallowRef<Mode2History | null>(null)
  const stockItems = shallowRef<StockItem[]>([])
  const historyLoading = ref(false)
  const selectLoading = ref(false)
  const historyError = ref('')
  const selectError = ref('')

  let historyRequestVersion = 0
  let selectRequestVersion = 0
  // 名单缓存：键 = 完整请求（含参数与日期），参数变更后自动失效
  const selectCache = new Map<string, StockItem[]>()

  function opFilter(): Mode2OpFilter {
    switch (filterType.value) {
      case 'Less':
        return { Less: threshold.value ?? 0 }
      case 'Greater':
        return { Greater: threshold.value ?? 0 }
      case 'Equal':
        return { Equal: threshold.value ?? 0 }
      default:
        return 'None'
    }
  }

  function buildSelectReq(date: string): SelectParams {
    return {
      field: field.value,
      direction: direction.value,
      filter: opFilter(),
      select: selectN.value ?? 10,
      profit_mode: profitMode.value,
      date,
      base: cloneBase(base),
    }
  }

  function buildHistoryReq(): HistoryParams {
    return {
      field: field.value,
      direction: direction.value,
      filter: opFilter(),
      select: selectN.value ?? 10,
      profit_mode: profitMode.value,
      base: cloneBase(base),
    }
  }

  function persistBase(): void {
    try {
      localStorage.setItem(FILTER_CACHE_KEY, JSON.stringify(cloneBase(base)))
    } catch {
      // localStorage full or unavailable
    }
  }

  /** 初始化：加载周期配置与记忆的股票池，缺省取第一个周期。 */
  async function init(): Promise<void> {
    try {
      periods.value = await fetchPeriods()
    } catch {
      // 无周期配置时保持空范围
    }
    const cached = loadCachedBase()
    if (cached?.start && cached?.end) {
      base.start = cached.start
      base.end = cached.end
      base.filter_bz = cached.filter_bz ?? false
      base.filter_st = cached.filter_st ?? false
      base.sector = cached.sector ?? []
      base.indice = cached.indice ?? []
    } else if (periods.value[0]) {
      base.start = periods.value[0].start
      base.end = periods.value[0].end
    }
  }

  /** 区间回测；参数变更后同时刷新当前日期名单（D8）。 */
  async function loadHistory(): Promise<void> {
    if (!base.start || !base.end) return
    const version = ++historyRequestVersion
    historyLoading.value = true
    historyError.value = ''
    try {
      const data = await fetchMode2History(buildHistoryReq())
      if (historyRequestVersion !== version) return
      history.value = data
      persistBase()
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
    field,
    direction,
    filterType,
    threshold,
    selectN,
    profitMode,
    base,
    currentDate,
    periods,
    history,
    stockItems,
    historyLoading,
    selectLoading,
    historyError,
    selectError,
    init,
    loadHistory,
    loadSelect,
  }
})

/** 交易日去重（回测首点为基线点，与首个交易日重复）。 */
export function uniqueDates(datetime: string[]): string[] {
  return [...new Set(datetime)]
}
