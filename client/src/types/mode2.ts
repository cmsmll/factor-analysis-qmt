import type { ModeFilter } from './mode1'

/** 因子字段（与后端 `operator::Field` 对齐）。 */
export type Mode2Field = 'Close' | 'DividendYield' | 'TotalMarket'

/** 排序方向（与后端 `operator::Direction` 对齐）。 */
export type Mode2Direction = 'Asc' | 'Desc'

/** 过滤类型（与后端 `operator::Filter` 对齐）。 */
export type Mode2FilterType = 'None' | 'Less' | 'Greater' | 'Equal'

/** 过滤条件：None 为字符串，阈值类为 `{ Less: number }` 对象。 */
export type Mode2OpFilter = 'None' | { Less: number } | { Greater: number } | { Equal: number }

/** 单日选股请求（POST /api/mode2/select）。 */
export interface SelectParams {
  field: Mode2Field
  direction: Mode2Direction
  filter: Mode2OpFilter
  select: number
  profit_mode: 1 | 2 | 3 | 4
  date?: string
  base: ModeFilter
}

/** 区间回测请求（POST /api/mode2/history）。 */
export interface HistoryParams {
  field: Mode2Field
  direction: Mode2Direction
  filter: Mode2OpFilter
  select: number
  profit_mode: 1 | 2 | 3 | 4
  base: ModeFilter
}

/** 单日选股名单条目（与后端 `StockItem` 对齐）。 */
export interface StockItem {
  code: string
  name: string
  factor: number
  change_percent: number
  is_st: boolean
  exchange: string
  tags: string[]
  open: number
  high: number
  low: number
  close: number
  volume: number
  amount: number
  turnover: number
}

/** 区间回测统计（与后端 `Mode2Stats` 对齐）。 */
export interface Mode2Stats {
  total_profit: number
  annualized: number
  max_drawdown: number
  win_rate: number
}

/** 区间回测结果（与后端 `Mode2History` 对齐）。 */
export interface Mode2History {
  datetime: string[]
  portfolio: number[]
  benchmark: number[]
  turnover: number[]
  count: number[]
  stats: Mode2Stats
}
