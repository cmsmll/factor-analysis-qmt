import { reactive } from 'vue'
import { PROFIT_MODE_KEY } from '@/stores/mode2'
import type { ProfitMode } from '@/types/mode1'

function readProfitMode(): ProfitMode {
  const saved = Number(localStorage.getItem(PROFIT_MODE_KEY) ?? '')
  return Number.isInteger(saved) && saved >= 1 && saved <= 4 ? (saved as ProfitMode) : 1
}

/**
 * 三看板共享的过滤状态(模块级单例):
 * 拆分后 mode1/mode2 页面各自加载数据,但过滤条件/收益模式保持共享与持久化,
 * 与拆分前 KanbanBoard 的跨看板共享行为一致。
 */
const state = reactive({
  period: '',
  profitMode: readProfitMode() as ProfitMode,
  start: '',
  end: '',
  filter_bz: false,
  filter_st: false,
  sector: [] as string[],
  indice: [] as string[],
})

export function useBoardFilterState() {
  return state
}

export function persistProfitMode(): void {
  try {
    localStorage.setItem(PROFIT_MODE_KEY, String(state.profitMode))
  } catch {
    // localStorage 不可用时忽略
  }
}

export const PROFIT_MODE_OPTIONS = [
  { label: '收益1：当天收盘买，第二天收盘卖', value: 1 },
  { label: '收益2：第二天开盘买，第二天收盘卖', value: 2 },
  { label: '收益3：第二天开盘买，第三天开盘卖', value: 3 },
  { label: '收益4：第二天开盘买，第三天收盘卖', value: 4 },
] as const

export function cloneBoardFilter(filter: typeof state): typeof state {
  return reactive({
    ...filter,
    sector: [...filter.sector],
    indice: [...filter.indice],
  })
}
