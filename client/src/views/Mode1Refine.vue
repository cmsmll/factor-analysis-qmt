<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { storeToRefs } from 'pinia'
import { useMode1Store } from '@/stores/mode1'
import { useMode1PreviewStore } from '@/stores/mode1Preview'
import { loadPreviewParams } from '@/stores/mode1'
import type { ModeListItem, Mode1Data } from '@/types/mode1'

defineOptions({ name: 'Mode1Refine' })

const route = useRoute()
const mode1Store = useMode1Store()
const previewStore = useMode1PreviewStore()
const { results, statuses, errors } = storeToRefs(previewStore)

const modeId = computed(() => {
  const value = route.params.id
  return Array.isArray(value) ? (value[0] ?? '') : (value ?? '')
})

const data = computed<ModeListItem | null>(() => {
  if (mode1Store.curr?.args.base.id === modeId.value) return mode1Store.curr
  const previewData = results.value[modeId.value] as Mode1Data | undefined
  if (previewData) return { args: { base: { id: modeId.value, count: previewData.count, filter: { start: '', end: '', filter_bz: false, filter_st: false, sector: [], indice: [] } } }, data: previewData }
  return null
})

async function loadDetail() {
  if (data.value) return

  const cachedParams = loadPreviewParams(modeId.value)
  if (!cachedParams) return

  const previewData = results.value[cachedParams.base.id] as Mode1Data | undefined
  if (previewData) {
    mode1Store.setCurr({ args: cachedParams, data: previewData })
    return
  }

  await previewStore.loadMode(cachedParams, true)
  const loaded = results.value[modeId.value] as Mode1Data | undefined
  if (loaded) mode1Store.setCurr({ args: cachedParams, data: loaded })
}

onMounted(() => { void loadDetail() })
</script>

<template>
  <div class="refine-layout">
    <pre v-if="data">{{ JSON.stringify(data, null, 2) }}</pre>
    <p v-else-if="statuses[modeId] === 'loading'">加载中...</p>
    <p v-else-if="errors[modeId]" class="error-msg">{{ errors[modeId] }}</p>
    <p v-else>暂无数据</p>
  </div>
</template>

<style scoped>
.refine-layout {
  padding: 24px;
  background: #fff;
  border-radius: 8px;
}

pre {
  white-space: pre-wrap;
  word-break: break-all;
  font-size: 13px;
  line-height: 1.6;
}

.error-msg {
  color: #d03050;
}
</style>
