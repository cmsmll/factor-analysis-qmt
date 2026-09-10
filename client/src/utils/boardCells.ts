import { h } from 'vue'

/** 涨跌配色:0/无值中性,正值红,负值绿。 */
export function rateColor(value: number | undefined): string {
  if (value === undefined || value === 0) return 'var(--ui-text-main)'
  return value > 0 ? 'var(--ui-color-up)' : 'var(--ui-color-down)'
}

export function formatPercent(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return '--'
  return `${(value * 100).toFixed(2)}%`
}

/** 百分比单元格(红涨绿跌)。 */
export function rateCell(value: number | undefined) {
  return h('span', { style: { color: rateColor(value) } }, formatPercent(value))
}

/** 换手率单元格:后端为小数(0.0229 = 2.29%),显示需 ×100。 */
export function turnoverCell(value: number | undefined) {
  const text = value === undefined || !Number.isFinite(value) ? '--' : `${(value * 100).toFixed(2)}%`
  return h('span', text)
}

/** 整数单元格。 */
export function integerCell(value: number | undefined) {
  return value === undefined ? h('span', '--') : h('span', String(value))
}

export function first<T>(values: readonly T[]): T | undefined {
  return values[0]
}

export function last<T>(values: readonly T[]): T | undefined {
  return values.length > 0 ? values[values.length - 1] : undefined
}

export function average(values: readonly number[] | undefined): number | undefined {
  if (!values || values.length === 0) return undefined
  const valid = values.filter(Number.isFinite)
  if (valid.length === 0) return undefined
  return valid.reduce((sum, value) => sum + value, 0) / valid.length
}
