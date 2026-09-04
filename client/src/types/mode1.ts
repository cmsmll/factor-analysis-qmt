export interface ApiResponse<T> {
  info: string
  code: number
  data: T
}

export interface ModeFilter {
  start: string
  end: string
  filter_bz: boolean
  filter_st: boolean
  sector: string[]
  indice: string[]
}

export interface Period {
  name: string
  start: string
  end: string
}

export interface ModeBase {
  id: string
  count: number
  filter: ModeFilter
}

export type CoreValue = string | number | boolean

export interface CoreArg<T extends CoreValue = CoreValue> {
  name: string
  value: T
}

export type ModeCore = Record<string, CoreArg>

export interface ModeRequest {
  base: ModeBase
  core?: ModeCore
  [key: string]: unknown
}

export interface ModeListItem {
  args: ModeRequest
  data: Mode1Data
}

export interface Profit {
  source: number[]
  total_profit: number
  total_net_value: number
  annualized_profit: number
}

export interface Mode1Data {
  id: string
  name: string
  info: string
  count: number
  factor: number[][]
  turnover_rate?: number[][]
  profit1: Profit[]
  profit2: Profit[]
  profit3: Profit[]
  profit4: Profit[]
  datetime: string[]
}

export type ProfitMode = 1 | 2 | 3 | 4
export type QuantileCount = 3 | 5 | 10
export type ModeLoadStatus = 'idle' | 'loading' | 'success' | 'error'

/** 当日财务字段（总市值/股本/股息率/同比；后四项可能缺失为 null）。 */
export interface Mode1DetailFinance {
  total_market: number
  dividend_yield: number
  total_shares: number | null
  float_shares: number | null
  float_market: number | null
  du_profit_rate: number | null
  inc_net_profit_rate: number | null
}

/** 单日分位明细中的一只股票（`POST /api/mode1/{id}/detail` 返回行）。 */
export interface Mode1DetailRow {
  code: string
  name: string
  exchange: string
  tags: string[]
  /** 因子值 */
  factor: number
  /** 前向收益 [p1, p2, p3, p4, 换手率] */
  profit: [number, number, number, number, number]
  datetime: string
  change_percent: number
  open: number
  close: number
  high: number
  low: number
  volume: number
  amount: number
  turnover: number
  is_st: boolean
  /** 当日财务字段 */
  finance: Mode1DetailFinance
}

/** 单日分位明细响应。 */
export interface Mode1QuantileDay {
  date: string
  count: number
  /** 分位 `0..count`；股票数不足分位数时各分位共享当日集合。 */
  quantiles: Mode1DetailRow[][]
}

/** 单日明细请求：因子参数 + 可选目标日期（缺省取筛选区间末交易日）。 */
export interface Mode1DetailRequest extends ModeRequest {
  date?: string
}

/** 指数历史收益点（GET /api/indice/history 返回项）。 */
export interface IndicePoint {
  /** 日期 YYYY-MM-DD */
  datetime: string
  /** 日收益率（等比后复权，小数） */
  profit: number
}
