<script setup lang="ts">
import { computed } from 'vue'
import type { EChartsOption } from 'echarts'

defineOptions({ name: 'PortfolioNavChart' })

const props = defineProps<{
  dates: string[]
  portfolio: number[]
  benchmark: number[]
}>()

const emit = defineEmits<{ (e: 'select-date', date: string): void }>()

const option = computed<EChartsOption>(() => ({
  tooltip: {
    trigger: 'axis',
    valueFormatter: (value) => Number(value).toFixed(2),
  },
  legend: { data: ['组合', '基准'], top: 0 },
  grid: { left: 8, right: 16, top: 36, bottom: 40, containLabel: true },
  xAxis: { type: 'category', data: props.dates, boundaryGap: false },
  yAxis: {
    type: 'value',
    scale: true,
    axisLabel: { formatter: (value: number) => Number(value).toFixed(2) },
  },
  dataZoom: [
    { type: 'inside', start: 0, end: 100 },
    {
      type: 'slider',
      height: 8,
      bottom: 10,
      disabled: true,
      showDetail: false,
      showDataShadow: false,
      brushSelect: false,
      handleSize: 0,
      moveHandleSize: 0,
      borderColor: 'transparent',
      backgroundColor: '#edf1f7',
      fillerColor: '#8aa4d6',
      handleStyle: { opacity: 0 },
      moveHandleStyle: { opacity: 0 },
      emphasis: {
        handleStyle: { opacity: 0 },
        moveHandleStyle: { opacity: 0 },
      },
    },
  ],
  series: [
    { name: '组合', type: 'line', data: props.portfolio, smooth: true, symbol: 'none' },
    {
      name: '基准',
      type: 'line',
      data: props.benchmark,
      smooth: true,
      symbol: 'none',
      lineStyle: { type: 'dashed' },
    },
  ],
}))

function onChartClick(params: unknown): void {
  const axisValue = (params as { axisValue?: string }).axisValue
  if (axisValue) emit('select-date', axisValue)
}
</script>

<template>
  <VChart class="chart-body" :option="option" autoresize @click="onChartClick" />
</template>

<style scoped>
.chart-body {
  width: 100%;
  height: 320px;
}
</style>
