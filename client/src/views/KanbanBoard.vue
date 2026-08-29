<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import { NButton, NForm, NFormItem, NInput, NSelect } from 'naive-ui'

import KanbanHeader from '@/components/common/KanbanHeader.vue'
import FactorDashboard from '@/views/FactorDashboard.vue'
import Mode2Microcap from '@/views/Mode2Microcap.vue'
import RefreshIcon from '@/assets/icons/refresh.svg'
import { fetchIndices, fetchSectors } from '@/api/mode1'
import { createModeFilter, loadCachedFilter, useMode1Store } from '@/stores/mode1'
import { useMode2Store } from '@/stores/mode2'
import { useGlobalFilterSelectorStore } from '@/stores/globalFilterSelector'
import { useGlobalLoadingStore } from '@/stores/globalLoading'
import { useGlobalMessageStore } from '@/stores/globalMessage'
import type { ModeFilter, Period, ProfitMode } from '@/types/mode1'

defineOptions({ name: 'KanbanBoard' })

const route = useRoute()
const router = useRouter()

// ── 看板列表与切换（页首标题/渐变随看板，URL query 同步）──
const kanbanList = [
  {
    title: '因子看盘可视化显示',
    subtitle: '多因子量化分析平台',
    key: 'factor',
    gradient: 'linear-gradient(135deg, #1a237e 0%, #283593 40%, #3949ab 100%)',
  },
  {
    title: '因子选股可视化显示',
    subtitle: '微盘股 · 市值最小400只 → 收盘价最低80只',
    key: 'microcap',
    gradient: 'linear-gradient(135deg, #1b5e20 0%, #2e7d32 40%, #43a047 100%)',
  },
]
const boardIndex = ref(getKanbanIndex(route.query.kanban))
const board = computed(() => kanbanList[boardIndex.value]!)

function getKanbanIndex(kanban: unknown): number {
  const key = Array.isArray(kanban) ? kanban[0] : kanban
  const index = kanbanList.findIndex((item) => item.key === key)
  return index >= 0 ? index : 0
}

function switchKanban(step: number) {
  boardIndex.value = (boardIndex.value + step + kanbanList.length) % kanbanList.length
  void router.replace({ query: { ...route.query, kanban: board.value.key } })
}

// ── 固定过滤区（页首下方常驻，模式一/模式二共用；数据各自隔离）──
const store = useMode1Store()
const mode2Store = useMode2Store()
const globalLoading = useGlobalLoadingStore()
const globalMessage = useGlobalMessageStore()
const filterSelector = useGlobalFilterSelectorStore()
const { periods, periodLoading, listLoading, listError, periodError } = storeToRefs(store)
const { visible: globalLoadingVisible } = storeToRefs(globalLoading)

const searchKeyword = ref('')
let settingInitialPeriod = false
const listFilter = reactive<ModeFilter>({
  start: '',
  end: '',
  filter_bz: false,
  filter_st: false,
  sector: [],
  indice: [],
})
const filters = reactive({
  period: '',
  profitMode: 1 as ProfitMode,
})
const sectorOptions = ref<string[]>()
const indiceOptions = ref<string[]>()
// 列表刷新版本号：驱动模式一表格区域重置分页
const listRevision = ref(0)

const periodOptions = computed(() =>
  periods.value.map((period) => ({ label: period.name, value: period.name })),
)
const profitModeOptions = [
  { label: '收益1：当天收盘买，第二天收盘卖', value: 1 },
  { label: '收益2：第二天开盘买，第二天收盘卖', value: 2 },
  { label: '收益3：第二天开盘买，第三天开盘卖', value: 3 },
  { label: '收益4：第二天开盘买，第三天收盘卖', value: 4 },
]

function cloneModeFilter(filter: ModeFilter): ModeFilter {
  return {
    ...filter,
    sector: [...filter.sector],
    indice: [...filter.indice],
  }
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}

async function reloadList(): Promise<void> {
  if (!listFilter.start || !listFilter.end) throw new Error('没有可用的时间周期配置')
  listRevision.value += 1
  await store.loadList(cloneModeFilter(listFilter))
  if (listError.value) throw new Error(listError.value)
}

async function reloadDashboard(): Promise<void> {
  try {
    await globalLoading.run(async () => {
      await reloadList()
    })
  } catch (error) {
    globalMessage.error(errorMessage(error, '获取模式一列表失败'))
  }
}

function resetListFilter(period: Period): void {
  Object.assign(listFilter, createModeFilter(period))
}

function applyPeriodToFilter(period: Period): void {
  listFilter.start = period.start
  listFilter.end = period.end
}

function syncMode2(): void {
  void mode2Store.applyPool({
    start: listFilter.start,
    end: listFilter.end,
    filter_bz: listFilter.filter_bz,
    filter_st: listFilter.filter_st,
    sector: listFilter.sector,
    indice: listFilter.indice,
  })
}

async function loadPeriod(name: string) {
  const period = periods.value.find((item) => item.name === name)
  if (!period) return
  applyPeriodToFilter(period)
  try {
    await globalLoading.run(async () => {
      await reloadList()
    })
  } catch (error) {
    globalMessage.error(errorMessage(error, '获取模式一列表失败'))
  }
  syncMode2()
}

async function selectSectors(): Promise<void> {
  try {
    if (!sectorOptions.value) {
      await globalLoading.run(async () => {
        sectorOptions.value = await fetchSectors()
      })
    }
    const result = await filterSelector.open({
      title: '行业板块',
      options: sectorOptions.value ?? [],
      selected: listFilter.sector,
    })
    if (result) {
      listFilter.sector = result
      syncMode2()
    }
  } catch (error) {
    globalMessage.error(errorMessage(error, '行业板块加载失败'))
  }
}

async function selectIndices(): Promise<void> {
  try {
    if (!indiceOptions.value) {
      await globalLoading.run(async () => {
        indiceOptions.value = await fetchIndices()
      })
    }
    const result = await filterSelector.open({
      title: '指数列表',
      options: indiceOptions.value ?? [],
      selected: listFilter.indice,
    })
    if (result) {
      listFilter.indice = result
      syncMode2()
    }
  } catch (error) {
    globalMessage.error(errorMessage(error, '指数列表加载失败'))
  }
}

watch(
  () => filters.period,
  (name) => {
    if (!name || settingInitialPeriod) return
    void loadPeriod(name)
  },
  { flush: 'sync' },
)

// 收益模式变更 → 模式一表格列（props 传导）与模式二回测/列表统计同步重算
watch(
  () => filters.profitMode,
  (mode) => {
    void mode2Store.applyProfitMode(mode as 1 | 2 | 3 | 4)
  },
)

async function initializeKanban(): Promise<void> {
  try {
    await globalLoading.run(async () => {
      await store.loadPeriods()
      if (periodError.value) throw new Error(periodError.value)

      const period = periods.value[0]
      if (!period) throw new Error('没有可用的时间周期配置')

      const cachedFilter = loadCachedFilter()
      if (cachedFilter) {
        settingInitialPeriod = true
        const matchingPeriod = periods.value.find(
          (p) => p.start === cachedFilter.start && p.end === cachedFilter.end,
        )
        filters.period = matchingPeriod?.name || period.name
        settingInitialPeriod = false
        Object.assign(listFilter, cachedFilter)
        await reloadList()
      } else {
        settingInitialPeriod = true
        filters.period = period.name
        settingInitialPeriod = false
        resetListFilter(period)
        await reloadList()
      }
    })
    syncMode2()
  } catch (error) {
    globalMessage.error(errorMessage(error, '获取模式一列表失败'))
  }
}

onMounted(() => void initializeKanban())
</script>

<template>
  <div class="kanban-shell">
    <!-- 页首：同一组件内切换看板（标题/渐变随看板） -->
    <KanbanHeader
      :title="board.title"
      :subtitle="board.subtitle"
      :gradient="board.gradient"
      @prev="switchKanban(-1)"
      @next="switchKanban(1)"
    />

    <!-- 固定过滤区：切换看板时保持不动 -->
    <div class="filter-bar">
      <NForm layout="inline" label-placement="left" size="small">
        <NFormItem v-if="board.key === 'factor'" label="因子搜索">
          <NInput
            v-model:value="searchKeyword"
            placeholder="输入因子名称搜索"
            clearable
            style="width: 180px"
          />
        </NFormItem>
        <NFormItem label="行业板块">
          <NButton size="small" class="selector-button" @click="selectSectors">
            {{ listFilter.sector.length ? `已选 ${listFilter.sector.length} 项` : '全部行业' }}
          </NButton>
        </NFormItem>
        <NFormItem label="指数列表">
          <NButton size="small" class="selector-button" @click="selectIndices">
            {{ listFilter.indice.length ? `已选 ${listFilter.indice.length} 项` : '全部指数' }}
          </NButton>
        </NFormItem>
        <NFormItem label="时间周期">
          <NSelect
            v-model:value="filters.period"
            :options="periodOptions"
            :loading="periodLoading && !globalLoadingVisible"
            :disabled="periodLoading || listLoading"
            style="width: 140px"
          />
        </NFormItem>
        <NFormItem label="收益模式">
          <NSelect
            v-model:value="filters.profitMode"
            :options="profitModeOptions"
            :consistent-menu-width="false"
            style="width: 260px"
          />
        </NFormItem>
        <NFormItem v-if="board.key === 'factor'" label="" class="reload-form-item">
          <NButton
            type="primary"
            color="#409eff"
            size="small"
            class="reload-btn"
            :loading="listLoading && !globalLoadingVisible"
            :disabled="periodLoading || listLoading"
            @click="reloadDashboard"
          >
            <template #icon><img :src="RefreshIcon" alt="" class="reload-icon" /></template>
            重载
          </NButton>
        </NFormItem>
      </NForm>
    </div>

    <!-- 表格区域：仅此处随看板切换 -->
    <div class="kanban-content">
      <FactorDashboard
        v-show="board.key === 'factor'"
        :search-keyword="searchKeyword"
        :profit-mode="filters.profitMode"
        :revision="listRevision"
        class="kanban-board"
      />
      <Mode2Microcap v-show="board.key === 'microcap'" class="kanban-board" />
    </div>
  </div>
</template>

<style scoped>
.kanban-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 32px;
  gap: 24px;
}

.filter-bar {
  background: #fff;
  padding: 16px 20px;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.filter-bar :deep(.n-form) {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 20px;
}

.filter-bar :deep(.n-form-item) {
  margin-bottom: 0;
}

.filter-bar :deep(.n-form-item-feedback-wrapper) {
  display: none;
}

.selector-button {
  min-width: 112px;
  color: #409eff;
}

.reload-form-item {
  margin-left: auto;
}

.reload-btn {
  min-width: 76px;
}

.reload-icon {
  width: 14px;
  height: 14px;
  filter: brightness(0) invert(1);
}

.kanban-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.kanban-board {
  height: 100%;
}
</style>
