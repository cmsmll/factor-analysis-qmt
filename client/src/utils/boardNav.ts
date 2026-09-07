/** 看板导航：行情(market) → mode1 → mode2 三看板循环。 */

const BOARD_ORDER = ['/market', '/mode1', '/mode2']

/** 由当前路径按 step(±1) 取下一看板路径（循环）。 */
export function boardStepPath(currentPath: string, step: number): string {
  const index = BOARD_ORDER.findIndex((path) => currentPath.startsWith(path))
  const base = index >= 0 ? index : 1
  const next = BOARD_ORDER[(base + step + BOARD_ORDER.length) % BOARD_ORDER.length]
  return next ?? '/market'
}
