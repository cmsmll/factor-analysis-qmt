<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import { NCard, NEmpty, NSpin } from 'naive-ui'

import PageTitleBar from '@/components/common/PageTitleBar.vue'
import PortfolioNavChart from '@/components/visualization/PortfolioNavChart.vue'
import TurnoverBarChart from '@/components/visualization/TurnoverBarChart.vue'
import { MODE2_STRATEGIES, useMode2Store } from '@/stores/mode2'

defineOptions({ name: 'Mode2Preview' })

const route = useRoute()
const router = useRouter()
const store = useMode2Store()
const { currentStrategyKey, history, historyLoading, historyError } = storeToRefs(store)

const currentStrategy = computed(
  () => MODE2_STRATEGIES.find((strategy) => strategy.key === currentStrategyKey.value) ?? MODE2_STRATEGIES[0]!,
)

function backToList(): void {
  void router.push('/mode2')
}

// 进入第三级明细页；曲线点击带日期（query.date 定位名单日期）
function goDetail(date?: string): void {
  void router.push({
    name: 'mode2-detail',
    params: { id: String(route.params.id ?? '') },
    query: date ? { date } : {},
  })
}

const stats = computed(() => history.value?.stats ?? null)

const previewAvgCount = computed(() => {
  const counts = history.value?.count.slice(1) ?? []
  return counts.length ? counts.reduce((sum, value) => sum + value, 0) / counts.length : 0
})

// 统计项：百分比两位小数；平均入选数为整数（与 mode1 stats-header 布局一致）
const statItems = computed(() => [
  { label: '总收益', value: `${((stats.value?.total_profit ?? 0) * 100).toFixed(2)}%` },
  { label: '年化收益', value: `${((stats.value?.annualized ?? 0) * 100).toFixed(2)}%` },
  { label: '最大回撤', value: `${((stats.value?.max_drawdown ?? 0) * 100).toFixed(2)}%` },
  { label: '胜率', value: `${((stats.value?.win_rate ?? 0) * 100).toFixed(2)}%` },
  { label: '平均入选数', value: String(Math.round(previewAvgCount.value)) },
])

// 预览路由参数（策略 id）→ 恢复上下文并加载该策略回测（结果按 id 缓存）
watch(
  () => route.params.id,
  async (id) => {
    if (!id || route.name !== 'mode2-preview') return
    const strategyId = String(id)
    if (!MODE2_STRATEGIES.some((strategy) => strategy.key === strategyId)) {
      void router.replace('/mode2')
      return
    }
    await store.ensureContext()
    await store.selectStrategy(strategyId)
  },
  { immediate: true },
)
</script>

<template>
  <div class="preview-layout">
    <!-- 页首：复用统一 PageTitleBar（返回 + 标题 + 明细按钮进入第三级明细页） -->
    <PageTitleBar
      :title="`${currentStrategy.name} · ${currentStrategy.desc}`"
      @back="backToList"
      @detail="goDetail()"
    />

    <!-- 统计区独立于图表区（不共容器） -->
    <div v-if="history" class="stats-header">
      <div v-for="stat in statItems" :key="stat.label" class="stat-item">
        <span class="stat-label">{{ stat.label }}</span>
        <span class="stat-value">{{ stat.value }}</span>
      </div>
    </div>

    <NSpin :show="historyLoading">
      <div v-if="historyError" class="error-tip">{{ historyError }}</div>
      <template v-else-if="history">
        <div class="chart-grid">
          <NCard title="组合 / 基准净值" size="small" class="chart-card">
            <PortfolioNavChart
              :dates="history.datetime"
              :portfolio="history.portfolio"
              :benchmark="history.benchmark"
              @select-date="goDetail"
            />
            <div class="chart-tip">点击曲线上的日期可进入该日名单明细；区间尾部 1-2 个交易日无未来收益数据。</div>
          </NCard>
          <NCard title="调仓换手率 / 入选数" size="small" class="chart-card">
            <TurnoverBarChart
              :dates="history.datetime"
              :turnover="history.turnover"
              :count="history.count"
            />
          </NCard>
        </div>
      </template>
      <NEmpty v-else description="暂无回测数据" class="empty-block" />
    </NSpin>
  </div>
</template>

<style scoped>
.preview-layout {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding: 32px;
  max-width: 1440px;
  margin: 0 auto;
  overflow-y: auto;
}

.stats-header {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 16px;
}

.stat-item {
  background: #fff;
  border-radius: 8px;
  padding: 16px 20px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.stat-label {
  font-size: 14px;
  color: rgb(31, 34, 37);
}

.stat-value {
  font-size: 20px;
  font-weight: 700;
  color: rgb(31, 34, 37);
}

.chart-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.chart-card {
  min-width: 0;
}

.chart-tip {
  margin-top: 4px;
  font-size: 12px;
  color: #909399;
}

.error-tip {
  padding: 12px;
  color: #d03050;
  font-size: 13px;
}

.empty-block {
  padding: 32px 0;
}
</style>
