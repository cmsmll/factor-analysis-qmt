import { request } from './client'

/** 行情快照行（GET /api/market/list；字段已归一：change_percent 小数、volume 股、turnover_rate 换手率小数）。 */
export interface MarketSnapshotRow {
  code: string
  name: string
  exchange: string
  datetime: string
  change_percent: number
  open: number
  close: number
  high: number
  low: number
  volume: number
  amount: number
  turnover_rate: number
  /** 所属行业/指数分类 */
  tags: string[]
}

/** K 线单日行（镜像参考工程：turnover=成交额元）。 */
export interface MarketKlineRow {
  datetime: string
  change_percent: number
  open: number
  close: number
  high: number
  low: number
  volume: number
  turnover: number
  turnover_rate: number
}

/** K 线响应（组件所需 shape：镜像参考工程 { nam, code, market_data }）。 */
export interface MarketKline {
  nam: string
  code: string
  /** 交易所（上交所/深交所/北交所），用于代码后缀 */
  exchange?: string
  market_data: MarketKlineRow[]
}

/** 后端直出的全量 Contract（Bar 的 profit 与 table 已跳过序列化）。 */
export interface MarketContract {
  start: string
  end: string
  metadata: {
    exchange: string
    name: string
    code: string
    listing_date: string
    members: string[]
  }
  bar: Array<{
    market: {
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
    }
    finance: Record<string, number | null>
  }>
}

/** 统一快照日全量行情列表;date(YYYY-MM-DD)缺省取末交易日。 */
export function fetchMarketList(date?: string): Promise<MarketSnapshotRow[]> {
  const query = date ? `?date=${encodeURIComponent(date)}` : ''
  return request<MarketSnapshotRow[]>(`/api/market/list${query}`)
}

/** 单股 K 线（code 为裸代码）。
 *
 * 后端返回全量 Contract（原始口径：change_percent 百分数、volume 手、turnover=换手率小数、
 * amount=成交额元）；此处适配为组件所需 shape（change_percent 小数、volume 股、
 * turnover=成交额、turnover_rate=换手率小数），组件与参考实现保持零改动。
 */
export async function fetchMarketKline(code: string): Promise<MarketKline> {
  const contract = await request<MarketContract>(`/api/market/${encodeURIComponent(code)}/kline`)
  return {
    nam: contract.metadata.name,
    code: contract.metadata.code,
    exchange: contract.metadata.exchange,
    market_data: contract.bar.map((bar) => ({
      datetime: bar.market.datetime,
      change_percent: bar.market.change_percent / 100,
      open: bar.market.open,
      close: bar.market.close,
      high: bar.market.high,
      low: bar.market.low,
      volume: bar.market.volume * 100,
      turnover: bar.market.amount,
      turnover_rate: bar.market.turnover,
    })),
  }
}
