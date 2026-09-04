import { request } from './client'
import type {
  IndicePoint,
  Mode1Data,
  Mode1DetailRequest,
  Mode1QuantileDay,
  ModeFilter,
  ModeListItem,
  ModeRequest,
  Period,
} from '@/types/mode1'

export async function fetchPeriods(): Promise<Period[]> {
  const data = await request<Period[]>('/api/period')

  if (!Array.isArray(data)) {
    throw new Error('时间周期配置格式不正确')
  }

  return data
}

export async function fetchSectors(): Promise<string[]> {
  return fetchFilterOptions('/api/sector', '行业板块')
}

export async function fetchIndices(): Promise<string[]> {
  return fetchFilterOptions('/api/indice', '指数列表')
}

export async function fetchIndiceHistory(name: string): Promise<IndicePoint[]> {
  const data = await request<IndicePoint[]>('/api/indice', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ name }),
  })

  if (!Array.isArray(data)) {
    throw new Error('指数历史数据格式不正确')
  }

  return data
}

async function fetchFilterOptions(path: string, label: string): Promise<string[]> {
  const data = await request<string[]>(path)

  if (!Array.isArray(data)) {
    throw new Error(`${label}数据格式不正确`)
  }

  return [...new Set(data)]
    .filter(Boolean)
    .sort((left, right) => left.localeCompare(right, 'zh-CN'))
}

export async function fetchMode1List(filter: ModeFilter): Promise<ModeListItem[]> {
  const data = await request<ModeListItem[]>('/api/mode1/list', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(filter),
  })

  if (!Array.isArray(data)) {
    throw new Error('模式一列表数据格式不正确')
  }

  return data
}

export function fetchMode1Data(params: ModeRequest): Promise<Mode1Data> {
  const id = encodeURIComponent(params.base.id)

  return request<Mode1Data>(`/api/mode1/${id}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(params),
  })
}

/** 查询因子目标日单日分位明细（date 缺省时后端取筛选区间末交易日）。 */
export function fetchMode1Detail(params: Mode1DetailRequest): Promise<Mode1QuantileDay> {
  const id = encodeURIComponent(params.base.id)

  return request<Mode1QuantileDay>(`/api/mode1/${id}/detail`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(params),
  })
}
