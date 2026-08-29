<script setup lang="ts">
import { computed } from 'vue'
import type { EChartsOption } from 'echarts'

defineOptions({ name: 'TurnoverBarChart' })

const props = defineProps<{
  dates: string[]
  turnover: number[]
  count: number[]
}>()

const option = computed<EChartsOption>(() => ({
  tooltip: { trigger: 'axis' },
  legend: { data: ['调仓换手率', '入选数'], top: 0 },
  grid: { left: 8, right: 16, top: 36, bottom: 24, containLabel: true },
  xAxis: { type: 'category', data: props.dates },
  yAxis: [
    { type: 'value', name: '换手率', max: 1, axisLabel: { formatter: (value: number) => `${(value * 100).toFixed(2)}%` } },
    { type: 'value', name: '入选数', splitLine: { show: false } },
  ],
  series: [
    {
      name: '调仓换手率',
      type: 'bar',
      data: props.turnover,
      yAxisIndex: 0,
      barMaxWidth: 12,
      tooltip: { valueFormatter: (value) => `${(Number(value) * 100).toFixed(2)}%` },
    },
    {
      name: '入选数',
      type: 'line',
      data: props.count,
      yAxisIndex: 1,
      smooth: true,
      symbol: 'none',
      tooltip: { valueFormatter: (value) => String(Number(value)) },
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
  height: 320px;
}
</style>
