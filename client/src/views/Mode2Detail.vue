<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'

import MicrocapList from '@/components/microcap/MicrocapList.vue'
import PageTitleBar from '@/components/common/PageTitleBar.vue'
import { MODE2_STRATEGIES, useMode2Store } from '@/stores/mode2'

defineOptions({ name: 'Mode2Detail' })

const route = useRoute()
const router = useRouter()
const store = useMode2Store()
const { currentDate } = storeToRefs(store)

const strategyId = computed(() => String(route.params.id ?? ''))
const strategy = computed(
  () => MODE2_STRATEGIES.find((item) => item.key === strategyId.value) ?? MODE2_STRATEGIES[0]!,
)

function backToPreview(): void {
  void router.push(`/mode2/${strategyId.value}`)
}

// 恢复上下文并加载策略；预览页曲线点击日期经 query.date 定位名单日期
watch(
  () => route.params.id,
  async (id) => {
    if (!id || route.name !== 'mode2-detail') return
    const strategyKey = String(id)
    if (!MODE2_STRATEGIES.some((item) => item.key === strategyKey)) {
      void router.replace('/mode2')
      return
    }
    await store.ensureContext()
    await store.selectStrategy(strategyKey)
    const date = route.query.date
    if (typeof date === 'string' && date) currentDate.value = date
  },
  { immediate: true },
)
</script>

<template>
  <div class="detail-layout">
    <!-- 第三级明细页标题：{策略名}·明细，复用 PageTitleBar -->
    <PageTitleBar :title="`${strategy.name}·明细`" :show-detail="false" @back="backToPreview" />
    <MicrocapList />
  </div>
</template>

<style scoped>
.detail-layout {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 32px;
  max-width: 1440px;
  margin: 0 auto;
}
</style>
