<script setup lang="ts">
import { computed } from 'vue'
import type { EChartsOption } from 'echarts'

import type { StockItem } from '@/types/mode2'

defineOptions({ name: 'SectorPieChart' })

const props = defineProps<{
  items: StockItem[]
}>()

const option = computed<EChartsOption>(() => {
  const counts = new Map<string, number>()
  for (const item of props.items) {
    for (const tag of item.tags) {
      counts.set(tag, (counts.get(tag) ?? 0) + 1)
    }
  }
  const data = [...counts.entries()]
    .map(([name, value]) => ({ name, value }))
    .sort((left, right) => right.value - left.value)
  return {
    tooltip: {
      trigger: 'item',
      formatter: (params: unknown) => {
        const item = params as { name: string; value: number; percent: number }
        return `${item.name}: ${item.value} (${item.percent.toFixed(2)}%)`
      },
    },
    legend: { type: 'scroll', bottom: 0 },
    series: [{ type: 'pie', radius: ['35%', '62%'], center: ['50%', '45%'], data }],
  }
})
</script>

<template>
  <VChart class="chart-body" :option="option" autoresize />
</template>

<style scoped>
.chart-body {
  width: 100%;
  height: 360px;
}
</style>
