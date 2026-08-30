import { computed, reactive, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import { fetchMode2History, fetchMode2Select } from '@/api/mode2'
import { loadCachedFilter, useMode1Store } from '@/stores/mode1'
import type { ModeFilter } from '@/types/mode1'
import type {
  HistoryParams,
  Mode2History,
  Mode2Stage,
  Mode2Strategy,
  SelectParams,
  StockItem,
} from '@/types/mode2'

const FILTER_CACHE_KEY = 'mode2-filter'

/** 收益模式持久化键（看板过滤区与独立预览页共用）。 */
export const PROFIT_MODE_KEY = 'mode2-profit-mode'

/** 微盘股预设：市值最小 400 只 → 其中收盘价最低 80 只。 */
export const MICROCAP_STAGES: Mode2Stage[] = [
  { field: 'TotalMarket', direction: 'Asc', filter: 'None', select: 400 },
  { field: 'Close', direction: 'Asc', filter: 'None', select: 80 },
]

/** 因子选股策略预设（前端静态定义，可扩展）。 */
export const MODE2_STRATEGIES: Mode2Strategy[] = [
  {
    key: 'microcap',
    name: '微盘股',
    desc: '市值最小的 400 只中，收盘价最低的 80 只',
    stages: MICROCAP_STAGES,
  },
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
  // 收益模式与当前预览策略（由固定过滤区收益模式与列表行点击驱动）
  const profitMode = ref<1 | 2 | 3 | 4>(1)
  const currentStrategyKey = ref(MODE2_STRATEGIES[0]!.key)

  // 结果与加载状态
  const history = shallowRef<Mode2History | null>(null)
  const stockItems = shallowRef<StockItem[]>([])
  // 列表页数据：每个策略的完整回测（stats + count，平均入选数由 count 计算）
  const strategyData = shallowRef<Record<string, Mode2History>>({})
  const historyLoading = ref(false)
  const selectLoading = ref(false)
  const statsLoading = ref(false)
  const historyError = ref('')
  const selectError = ref('')

  let historyRequestVersion = 0
  let selectRequestVersion = 0
  // 名单缓存：键 = 完整请求（含股票池与日期），池变更后自动失效
  const selectCache = new Map<string, StockItem[]>()
  // 回测结果缓存（与 mode1 按 args 哈希 id 缓存一致）：键 = 完整请求；并发去重
  const historyCache = new Map<string, Mode2History>()
  const historyPending = new Map<string, Promise<Mode2History>>()

  /** 回测结果缓存读取：命中直接返回，未命中发起请求并去重。 */
  function fetchHistoryCached(req: HistoryParams): Promise<Mode2History> {
    const key = JSON.stringify(req)
    const cached = historyCache.get(key)
    if (cached) return Promise.resolve(cached)
    const pending = historyPending.get(key)
    if (pending) return pending
    const task = fetchMode2History(req).then((data) => {
      historyCache.set(key, data)
      return data
    })
    historyPending.set(key, task)
    void task.finally(() => {
      if (historyPending.get(key) === task) historyPending.delete(key)
    })
    return task
  }

  const currentStrategy = computed(
    () => MODE2_STRATEGIES.find((strategy) => strategy.key === currentStrategyKey.value) ?? MODE2_STRATEGIES[0]!,
  )

  function buildSelectReq(date: string): SelectParams {
    return {
      stages: currentStrategy.value.stages,
      profit_mode: profitMode.value,
      date,
      base: cloneBase(base),
    }
  }

  function buildHistoryReq(): HistoryParams {
    return {
      stages: currentStrategy.value.stages,
      profit_mode: profitMode.value,
      base: cloneBase(base),
    }
  }

  /** 由看板固定过滤区驱动：更新股票池并刷新列表统计、回测与当前日期名单。 */
  async function applyPool(pool: {
    start: string
    end: string
    filter_bz: boolean
    filter_st: boolean
    sector: string[]
    indice: string[]
  }): Promise<void> {
    base.start = pool.start
    base.end = pool.end
    base.filter_bz = pool.filter_bz
    base.filter_st = pool.filter_st
    base.sector = [...pool.sector]
    base.indice = [...pool.indice]
    try {
      localStorage.setItem(FILTER_CACHE_KEY, JSON.stringify(cloneBase(base)))
    } catch {
      // localStorage full or unavailable
    }
    await Promise.all([loadListStats(), loadHistory()])
  }

  /** 收益模式变更：重算全部策略列表统计与当前策略预览回测。 */
  async function applyProfitMode(mode: 1 | 2 | 3 | 4): Promise<void> {
    if (profitMode.value === mode) return
    profitMode.value = mode
    await Promise.all([loadListStats(), loadHistory()])
  }

  /** 选择预览策略：加载该策略的回测与名单（结果命中缓存则不重新请求）。 */
  async function selectStrategy(key: string): Promise<void> {
    currentStrategyKey.value = key
    await loadHistory()
  }

  /** 独立页直达/刷新：从持久化恢复股票池与收益模式；无缓存时取第一个周期为默认池。 */
  async function ensureContext(): Promise<void> {
    if (!base.start || !base.end) {
      const cached = loadCachedFilter()
      if (cached?.start && cached?.end) {
        await applyPool({
          start: cached.start,
          end: cached.end,
          filter_bz: cached.filter_bz ?? false,
          filter_st: cached.filter_st ?? false,
          sector: cached.sector ?? [],
          indice: cached.indice ?? [],
        })
      } else {
        const mode1Store = useMode1Store()
        await mode1Store.loadPeriods()
        const period = mode1Store.periods[0]
        if (period) {
          await applyPool({
            start: period.start,
            end: period.end,
            filter_bz: false,
            filter_st: false,
            sector: [],
            indice: [],
          })
        }
      }
    }
    const savedMode = Number(localStorage.getItem(PROFIT_MODE_KEY) ?? '')
    if (Number.isInteger(savedMode) && savedMode >= 1 && savedMode <= 4) {
      await applyProfitMode(savedMode as 1 | 2 | 3 | 4)
    }
  }

  /** 预览页 ST/北证过滤切换：刷新列表统计与当前策略回测/名单。 */
  async function setStFilter(field: 'filter_bz' | 'filter_st', value: boolean): Promise<void> {
    if (base[field] === value) return
    base[field] = value
    await Promise.all([loadListStats(), loadHistory()])
  }

  /** 列表数据：逐策略调 history（当前股票池 + 收益模式），供列表统计与平均入选数使用。 */
  async function loadListStats(force = false): Promise<void> {
    if (!base.start || !base.end) return
    statsLoading.value = true
    try {
      const results = await Promise.all(
        MODE2_STRATEGIES.map(async (strategy) => {
          const req = {
            stages: strategy.stages,
            profit_mode: profitMode.value,
            base: cloneBase(base),
          }
          const data = force ? await fetchMode2History(req) : await fetchHistoryCached(req)
          return [strategy.key, data] as const
        }),
      )
      strategyData.value = Object.fromEntries(results)
    } catch {
      // 统计加载失败时保留旧值
    } finally {
      statsLoading.value = false
    }
  }

  /** 区间回测；股票池变更后同步刷新当前日期名单（结果命中缓存则不重新请求）。 */
  async function loadHistory(): Promise<void> {
    if (!base.start || !base.end) return
    const version = ++historyRequestVersion
    historyLoading.value = true
    historyError.value = ''
    try {
      const data = await fetchHistoryCached(buildHistoryReq())
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
    profitMode,
    currentStrategyKey,
    currentStrategy,
    history,
    stockItems,
    strategyData,
    historyLoading,
    selectLoading,
    statsLoading,
    historyError,
    selectError,
    applyPool,
    applyProfitMode,
    selectStrategy,
    setStFilter,
    ensureContext,
    loadListStats,
    loadHistory,
    loadSelect,
  }
})

/** 交易日去重（回测首点为基线点，与首个交易日重复）。 */
export function uniqueDates(datetime: string[]): string[] {
  return [...new Set(datetime)]
}
