<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { getInstanceByDom } from 'echarts/core'
import type { EChartsOption } from 'echarts'

defineOptions({ name: 'PortfolioNavChart' })

const props = defineProps<{
  dates: string[]
  portfolio: number[]
  benchmark: number[]
}>()

const emit = defineEmits<{ (e: 'select-date', date: string): void }>()

const wrapRef = ref<HTMLElement>()

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

// vue-echarts 事件转发在此环境不可靠：改用原生点击 + ECharts 像素反查日期。
function onWrapClick(event: MouseEvent): void {
  const wrap = wrapRef.value
  const chartDom = wrap?.querySelector<HTMLElement>('.echarts') ?? wrap?.firstElementChild
  if (!wrap || !chartDom) return
  const chart = getInstanceByDom(chartDom as HTMLElement)
  if (!chart) return
  const rect = wrap.getBoundingClientRect()
  const point = chart.convertFromPixel(
    { seriesIndex: 0 },
    [event.clientX - rect.left, event.clientY - rect.top],
  )
  const value = point?.[0]
  // category 轴 convertFromPixel 返回索引（number），fallback 兼容字符串值
  if (typeof value === 'number' && props.dates[value]) {
    emit('select-date', props.dates[value])
  } else if (typeof value === 'string' && value) {
    emit('select-date', value)
  }
}

onMounted(() => {
  wrapRef.value?.addEventListener('click', onWrapClick)
})

onBeforeUnmount(() => {
  wrapRef.value?.removeEventListener('click', onWrapClick)
})
</script>

<template>
  <div ref="wrapRef" class="chart-body">
    <VChart :option="option" autoresize />
  </div>
</template>

<style scoped>
.chart-body {
  width: 100%;
  height: 320px;
}
</style>
