<script setup lang="ts">
import { computed } from 'vue'
import type { EChartsOption } from 'echarts'

import type { StockItem } from '@/types/mode2'

defineOptions({ name: 'FactorRankChart' })

const props = defineProps<{
  items: StockItem[]
}>()

// 按因子值降序，第一名在顶部（条形图自下而上，需反转数组）。
const sorted = computed(() => [...props.items].sort((left, right) => right.factor - left.factor))

const option = computed<EChartsOption>(() => ({
  tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
  grid: { left: 8, right: 24, top: 16, bottom: 8, containLabel: true },
  xAxis: { type: 'value' },
  yAxis: {
    type: 'category',
    data: sorted.value.map((item) => `${item.code} ${item.name}`),
    inverse: true,
  },
  series: [
    {
      name: '因子值',
      type: 'bar',
      data: sorted.value.map((item) => item.factor),
      barMaxWidth: 14,
    },
  ],
}))
</script>

<template>
  <VChart class="chart-body" :option="option" autoresize />
</template>

<style scoped>
.chart-body {
  width: 100%;
  height: 240px;
}
</style>
