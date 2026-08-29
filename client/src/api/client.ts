import type { ApiResponse } from '@/types/mode1'

const apiBaseUrl = (import.meta.env.VITE_API_BASE_URL ?? '').replace(/\/$/, '')

/** 统一请求封装：提取 `{ code, info, data }` 响应中的 `data`，失败时抛出 `Error`。 */
export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${apiBaseUrl}${path}`, init)
  const payload = (await response.json()) as ApiResponse<T>

  if (!response.ok || payload.code >= 400) {
    throw new Error(payload.info || `请求失败（${response.status}）`)
  }

  return payload.data
}
