import { request } from './client'
import type { HistoryParams, Mode2History, SelectParams, StockItem } from '@/types/mode2'

/** 区间回测：逐日选股、组合/基准净值、调仓换手率与统计。 */
export function fetchMode2History(params: HistoryParams): Promise<Mode2History> {
  return request<Mode2History>('/api/mode2/history', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  })
}

/** 单日选股名单。 */
export function fetchMode2Select(params: SelectParams): Promise<StockItem[]> {
  return request<StockItem[]>('/api/mode2/select', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  })
}
