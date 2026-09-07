import type { Time } from 'lightweight-charts'

/**
 * 从行情数据的 datetime 字段中提取交易日期。
 *
 * 原始数据格式通常是 `YYYY-MM-DD HH:mm:ss`，日 K 图只需要日期部分。
 * 如果字符串中没有空格，则返回原值作为兜底。
 */
export function toChartDate(datetime: string): string {
  const date = datetime.split(' ')[0]
  return date || datetime
}

/**
 * 将 Lightweight Charts 返回的时间值转换为 `YYYY-MM-DD` 日期字符串。
 *
 * 当前日 K 数据使用字符串日期作为 time；如果图表库返回 BusinessDay 对象，
 * 这里也会兼容转换。数字时间戳当前未用于日 K 查询，所以返回 null。
 */
export function chartTimeToDate(time: Time | undefined): string | null {
  if (!time) return null
  if (typeof time === 'string') return time
  if (typeof time === 'number') return null

  return `${time.year}-${String(time.month).padStart(2, '0')}-${String(time.day).padStart(2, '0')}`
}

/**
 * 格式化行情数据中的日期时间字符串。
 *
 * 原始格式是 `YYYY-MM-DD HH:mm:ss`，十字光标底部标签中显示到分钟即可，
 * 例如 `2024-01-02 09:30:00` 会显示为 `2024-01-02 09:30`。
 */
export function formatDateTime(datetime: string): string {
  const [date, time] = datetime.split(' ')
  if (!date || !time) return datetime

  return `${date} ${time.slice(0, 5)}`
}

/**
 * 格式化价格，统一保留两位小数。
 */
export function formatPrice(value: number): string {
  return value.toFixed(2)
}

/**
 * 格式化带正负号的百分比。
 *
 * 行情数据中涨跌幅是小数比例，例如 `0.0123` 表示 `+1.23%`。
 */
export function formatPercent(value: number): string {
  const percent = value * 100
  return `${percent >= 0 ? '+' : ''}${percent.toFixed(2)}%`
}

/**
 * 格式化不带正号的普通百分比。
 *
 * 适用于换手率这类非涨跌方向指标，例如 `0.029` 显示为 `2.90%`。
 */
export function formatPlainPercent(value: number): string {
  return `${(value * 100).toFixed(2)}%`
}

/**
 * 格式化较大的成交量/成交额数字。
 *
 * - 大于等于 1 亿：使用“亿”作为单位
 * - 大于等于 1 万：使用“万”作为单位
 * - 其他数值：显示整数并使用中文千分位
 */
export function formatLargeNumber(value: number): string {
  const abs = Math.abs(value)

  if (abs >= 100000000) {
    return `${(value / 100000000).toFixed(2)}亿`
  }

  if (abs >= 10000) {
    return `${(value / 10000).toFixed(2)}万`
  }

  return Math.round(value).toLocaleString('zh-CN')
}
