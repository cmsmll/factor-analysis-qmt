<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import StockKLine from '@/components/market/StockKLine.vue'

defineOptions({ name: 'MarketKLine' })

const route = useRoute()

const code = computed(() => {
  const value = route.params.code
  return Array.isArray(value) ? (value[0] ?? '') : (value ?? '')
})

/** 行情日期（YYYY-MM-DD），用于 K 线按该日 ±半年定位。 */
const centerDate = computed(() => {
  const value = route.query.date
  return typeof value === 'string' && value ? value : undefined
})
</script>

<template>
  <StockKLine :code="code" :center-date="centerDate" />
</template>
