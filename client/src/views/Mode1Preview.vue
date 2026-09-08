<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useRoute, useRouter } from 'vue-router'
import UiButton from '@/components/ui/UiButton.vue'
import UiDatePicker from '@/components/ui/UiDatePicker.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiInputNumber from '@/components/ui/UiInputNumber.vue'
import UiRadio from '@/components/ui/UiRadio.vue'
import UiRadioGroup from '@/components/ui/UiRadioGroup.vue'
import UiSelect from '@/components/ui/UiSelect.vue'
import PageTitleBar from '@/components/common/PageTitleBar.vue'
import RefreshIcon from '@/assets/icons/refresh.svg'
import { fetchDataRange, fetchIndices, fetchSectors } from '@/api/mode1'
import DecayChart from '@/components/visualization/DecayChart.vue'
import FactorMetrics from '@/components/visualization/FactorMetrics.vue'
import GroupNavChart from '@/components/visualization/GroupNavChart.vue'
import IcChart from '@/components/visualization/IcChart.vue'
import IndustryIcChart from '@/components/visualization/IndustryIcChart.vue'
import TurnoverChart from '@/components/visualization/TurnoverChart.vue'
import { buildDetail } from '@/features/mode1/refine'
import { useGlobalLoadingStore } from '@/stores/globalLoading'
import { useGlobalMessageStore } from '@/stores/globalMessage'
import { useGlobalFilterSelectorStore } from '@/stores/globalFilterSelector'
import { loadPreviewParams, savePreviewParams, useMode1Store } from '@/stores/mode1'
import { useMode1PreviewStore } from '@/stores/mode1Preview'
import type {
  CoreArg,
  CoreValue,
  ModeFilter,
  ModeListItem,
  ModeRequest,
  ProfitMode,
  QuantileCount,
} from '@/types/mode1'
import { formatDate } from '@/utils/factorSeries'

defineOptions({ name: 'Mode1Preview' })

const route = useRoute()
const router = useRouter()
const mode1Store = useMode1Store()
const previewStore = useMode1PreviewStore()
const globalLoading = useGlobalLoadingStore()
const globalMessage = useGlobalMessageStore()
const filterSelector = useGlobalFilterSelectorStore()
const { items, listError, periodError } = storeToRefs(mode1Store)
const { results, statuses, errors } = storeToRefs(previewStore)
const { visible: globalLoadingVisible } = storeToRefs(globalLoading)
const requestParams = ref<ModeRequest>()
const startDate = ref<number | null>(null)
const endDate = ref<number | null>(null)
const initialStartDate = ref<number | null>(null)
const initialEndDate = ref<number | null>(null)
const profitMode = ref<ProfitMode>(1)
const quantileCount = ref<number>(5)
const sectorOptions = ref<string[]>()
const indiceOptions = ref<string[]>()
let settingRequestDates = false
type CoreInputType = 'boolean' | 'number' | 'text'

interface CoreParamItem {
  key: string
  name: string
  value: CoreValue
  inputType: CoreInputType
  integer: boolean
}
const profitModeOptions = [
  { label: '收益1：当天收盘买，第二天收盘卖', value: 1 },
  { label: '收益2：第二天开盘买，第二天收盘卖', value: 2 },
  { label: '收益3：第二天开盘买，第三天开盘卖', value: 3 },
  { label: '收益4：第二天开盘买，第三天收盘卖', value: 4 },
]

const modeId = computed(() => {
  const value = route.params.id
  return Array.isArray(value) ? (value[0] ?? '') : (value ?? '')
})
const factorData = computed(() => results.value[modeId.value])
const quantileLoading = computed(() => statuses.value[modeId.value] === 'loading')
const localQuantileLoading = computed(() => quantileLoading.value && !globalLoadingVisible.value)
const routeFactorName = computed(() => factorData.value?.name || '因子预览')
const routeFactorInfo = computed(() => factorData.value?.info || routeFactorName.value)
const factorDetail = computed(() => buildDetail(factorData.value, null, null, profitMode.value))
const stats = computed(() => factorDetail.value.stats)
const previewFilter = computed(() => requestParams.value?.base.filter)
const coreParams = computed<CoreParamItem[]>(() => {
  const core = requestParams.value?.core
  if (!core) return []

  return Object.entries(core).flatMap(([key, arg]) => {
    if (!isCoreArg(arg)) return []

    return [
      {
        key,
        name: arg.name || key,
        value: arg.value,
        inputType: coreInputType(arg.value),
        integer: typeof arg.value === 'number' && Number.isInteger(arg.value),
      },
    ]
  })
})

watch(
  () => modeId.value,
  () => {
    requestParams.value = undefined
    startDate.value = null
    endDate.value = null
    initialStartDate.value = null
    initialEndDate.value = null
    quantileCount.value = 5
    void loadPreview()
  },
  { immediate: true },
)

watch(
  startDate,
  (value) => {
    if (settingRequestDates || value === null || endDate.value === null) return
    if (value > endDate.value) endDate.value = value
    syncRequestDates(value, endDate.value)
  },
  { flush: 'sync' },
)

watch(
  endDate,
  (value) => {
    if (settingRequestDates || value === null || startDate.value === null) return
    if (value < startDate.value) startDate.value = value
    syncRequestDates(startDate.value, value)
  },
  { flush: 'sync' },
)

async function loadPreview() {
  try {
    await globalLoading.run(async () => {
      const cachedParams = loadPreviewParams(modeId.value)
      // 列表页当前项优先：缓存与本次列表区间不一致时（旧缓存失效），跟随列表并覆盖缓存
      const current = mode1Store.curr
      const currentParams =
        current?.args?.base?.id === modeId.value ? current.args : null
      const cacheStale =
        cachedParams !== null &&
        currentParams !== null &&
        !sameFilter(cachedParams.base.filter, currentParams.base.filter)
      if (cachedParams && !cacheStale) {
        requestParams.value = cachedParams
        quantileCount.value = cachedParams.base.count
        setRequestDates(cachedParams)
        await previewStore.loadMode(cachedParams, true)
        const loaded = results.value[modeId.value]
        if (loaded) mode1Store.setCurr({ args: cachedParams, data: loaded })
        if (statuses.value[modeId.value] === 'error') {
          throw new Error(errors.value[modeId.value] ?? '因子数据加载失败')
        }
        return
      }
      await mode1Store.loadDefaultList()
      if (periodError.value) throw new Error(periodError.value)
      if (listError.value) throw new Error(listError.value)

      const source = findPreviewSource()
      if (!source) throw new Error('没有找到对应的因子列表数据')

      const params = structuredClone(source.args)
      requestParams.value = params
      quantileCount.value = params.base.count
      setRequestDates(params)
      previewStore.initialize(modeId.value, source.data)
      savePreviewParams(modeId.value, params)
      mode1Store.setCurr(source)
    })
  } catch (error) {
    globalMessage.error(errorMessage(error, '因子数据加载失败'))
  }
}

function isQuantileCount(value: number): value is QuantileCount {
  return value === 3 || value === 5 || value === 10
}

function isCoreArg(value: unknown): value is CoreArg {
  if (!value || typeof value !== 'object') return false
  const arg = value as Partial<CoreArg>
  const valueType = typeof arg.value
  return (
    typeof arg.name === 'string' &&
    (valueType === 'string' || valueType === 'number' || valueType === 'boolean')
  )
}

function coreInputType(value: CoreValue): CoreInputType {
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'number') return 'number'
  return 'text'
}

function findPreviewSource(): ModeListItem | undefined {
  if (mode1Store.curr?.args?.base?.id === modeId.value) return mode1Store.curr

  const found = mode1Store.items.find((item) => item.args.base.id === modeId.value)
  if (found) mode1Store.setCurr(found)
  return found
}

function sameFilter(left: ModeFilter, right: ModeFilter): boolean {
  return (
    left.start === right.start &&
    left.end === right.end &&
    left.filter_bz === right.filter_bz &&
    left.filter_st === right.filter_st &&
    left.sector.length === right.sector.length &&
    left.sector.every((item, index) => item === right.sector[index]) &&
    left.indice.length === right.indice.length &&
    left.indice.every((item, index) => item === right.indice[index])
  )
}

function updateCoreParam(key: string, value: CoreValue | null): void {
  const target = requestParams.value?.core?.[key]
  if (!isCoreArg(target)) return

  if (typeof target.value === 'number') {
    if (typeof value !== 'number' || !Number.isFinite(value)) return
    target.value = Number.isInteger(target.value) ? Math.trunc(value) : value
    return
  }

  if (typeof target.value === 'boolean') {
    if (typeof value === 'boolean') target.value = value
    return
  }

  if (typeof value === 'string') target.value = value
}

async function changeQuantileCount(value: number) {
  if (!isQuantileCount(value) || quantileLoading.value) return

  const params = requestParams.value
  if (!params || (factorData.value?.count === value && quantileCount.value === value)) return

  const previousCount = factorData.value?.count
  quantileCount.value = value
  params.base.count = value
  try {
    await globalLoading.run(async () => {
      await previewStore.loadMode(params, true)

      const typeId = params.base.id
      if (typeId !== modeId.value && results.value[typeId]) {
        results.value[modeId.value] = results.value[typeId]
        statuses.value[modeId.value] = 'success'
      }

      if (statuses.value[modeId.value] === 'error') {
        throw new Error(errors.value[modeId.value] ?? '分位数据计算失败')
      }
    })
  } catch (error) {
    if (previousCount && isQuantileCount(previousCount)) {
      quantileCount.value = previousCount
      params.base.count = previousCount
    }
    globalMessage.error(errorMessage(error, `${quantileLabel(value)}数据计算失败`))
  }
}

async function reloadPreview(): Promise<void> {
  const params = requestParams.value
  if (!params || quantileLoading.value) return

  try {
    await requestPreview(params, '因子数据重载失败')
  } catch (error) {
    globalMessage.error(errorMessage(error, '因子数据重载失败'))
  }
}

async function requestPreview(params: ModeRequest, fallback: string): Promise<void> {
  await globalLoading.run(async () => {
    await previewStore.loadMode(params, true)

    // loadMode 用 params.base.id (TypeId) 存结果，拷贝到 modeId (data.id) 键下
    const typeId = params.base.id
    if (typeId !== modeId.value && results.value[typeId]) {
      results.value[modeId.value] = results.value[typeId]
      statuses.value[modeId.value] = 'success'
    }

    if (statuses.value[modeId.value] === 'error') {
      throw new Error(errors.value[modeId.value] ?? fallback)
    }

    savePreviewParams(modeId.value, params)
    const loaded = results.value[modeId.value]
    if (loaded) mode1Store.setCurr({ args: params, data: loaded })
  })
}

async function selectSectors(): Promise<void> {
  const filter = previewFilter.value
  if (!filter) return

  try {
    if (!sectorOptions.value) {
      await globalLoading.run(async () => {
        sectorOptions.value = await fetchSectors()
      })
    }
    const result = await filterSelector.open({
      title: '行业板块',
      options: sectorOptions.value ?? [],
      selected: filter.sector,
    })
    if (result) filter.sector = result
  } catch (error) {
    globalMessage.error(errorMessage(error, '行业板块加载失败'))
  }
}

async function selectIndices(): Promise<void> {
  const filter = previewFilter.value
  if (!filter) return

  try {
    if (!indiceOptions.value) {
      await globalLoading.run(async () => {
        indiceOptions.value = await fetchIndices()
      })
    }
    const result = await filterSelector.open({
      title: '指数列表',
      options: indiceOptions.value ?? [],
      selected: filter.indice,
    })
    if (result) filter.indice = result
  } catch (error) {
    globalMessage.error(errorMessage(error, '指数列表加载失败'))
  }
}

function quantileLabel(value: QuantileCount): string {
  if (value === 3) return '三分位'
  if (value === 5) return '五分位'
  return '十分位'
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}
function toDateValue(value?: string): number | null {
  if (!value) return null
  const timestamp = new Date(formatDate(value)).getTime()
  return Number.isFinite(timestamp) ? timestamp : null
}

function resetStartDate() {
  resetRequestDates()
}

function resetEndDate() {
  resetRequestDates()
}

function setRequestDates(params: ModeRequest) {
  settingRequestDates = true
  startDate.value = toDateValue(params.base.filter.start)
  endDate.value = toDateValue(params.base.filter.end)
  initialStartDate.value = startDate.value
  initialEndDate.value = endDate.value
  settingRequestDates = false
}

function syncRequestDates(start: number, end: number): void {
  const filter = previewFilter.value
  if (!filter) return
  filter.start = toDateString(start)
  filter.end = toDateString(end)
}

function toDateString(timestamp: number): string {
  const date = new Date(timestamp)
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function resetRequestDates() {
  settingRequestDates = true
  startDate.value = initialStartDate.value
  endDate.value = initialEndDate.value
  settingRequestDates = false
  if (startDate.value !== null && endDate.value !== null) {
    syncRequestDates(startDate.value, endDate.value)
  }
}

function goBack(): void {
  void router.push({ name: 'mode1' })
}

/** 进入单日分位明细页；图表点击带目标日期（query.date 定位）。 */
function goDetail(date?: string): void {
  void router.push({
    name: 'mode1-detail',
    params: { id: modeId.value },
    query: date ? { date } : {},
  })
}

/** 日历可选边界(毫秒,全市场数据区间;加载失败不限界)。 */
const rangeMinMs = ref<number | undefined>(undefined)
const rangeMaxMs = ref<number | undefined>(undefined)

onMounted(() => {
  fetchDataRange()
    .then((range) => {
      const min = toDateValue(range.min_date)
      const max = toDateValue(range.max_date)
      if (min !== null) rangeMinMs.value = min
      if (max !== null) rangeMaxMs.value = max
    })
    .catch(() => {
      // 区间获取失败:日历保持不限界,不阻塞页面
    })
})
</script>

<template>
  <div class="mode1-preview">
    <!-- 页首 -->
    <PageTitleBar
      :title="routeFactorName || factorDetail.factorName"
      :title-tip="routeFactorInfo"
      :show-detail="false"
      @back="goBack"
    />

    <div v-if="previewFilter" class="filter-bar">
      <div class="filter-form">
        <div class="filter-item">
          <span class="filter-item__label">过滤ST</span>
          <UiRadioGroup v-model:value="previewFilter.filter_st" size="small">
            <UiRadio type="button" :value="false">否</UiRadio>
            <UiRadio type="button" :value="true">是</UiRadio>
          </UiRadioGroup>
        </div>
        <div class="filter-item">
          <span class="filter-item__label">过滤北证</span>
          <UiRadioGroup v-model:value="previewFilter.filter_bz" size="small">
            <UiRadio type="button" :value="false">否</UiRadio>
            <UiRadio type="button" :value="true">是</UiRadio>
          </UiRadioGroup>
        </div>
        <div class="filter-item">
          <span class="filter-item__label">行业板块</span>
          <UiButton size="small" class="selector-button" @click="selectSectors">
            {{
              previewFilter.sector.length ? `已选 ${previewFilter.sector.length} 项` : '全部行业'
            }}
          </UiButton>
        </div>
        <div class="filter-item">
          <span class="filter-item__label">指数列表</span>
          <UiButton size="small" class="selector-button" @click="selectIndices">
            {{
              previewFilter.indice.length ? `已选 ${previewFilter.indice.length} 项` : '全部指数'
            }}
          </UiButton>
        </div>
        <div v-for="param in coreParams" :key="param.key" class="filter-item">
          <span class="filter-item__label">{{ param.name }}</span>
          <UiInputNumber
            v-if="param.inputType === 'number'"
            :value="param.value as number"
            size="small"
            class="core-input"
            @update:value="(value) => updateCoreParam(param.key, value)"
          />
          <UiRadioGroup
            v-else-if="param.inputType === 'boolean'"
            :value="param.value as boolean"
            size="small"
            @update:value="(value) => updateCoreParam(param.key, value as boolean)"
          >
            <UiRadio type="button" :value="false">否</UiRadio>
            <UiRadio type="button" :value="true">是</UiRadio>
          </UiRadioGroup>
          <UiInput
            v-else
            :value="param.value as string"
            size="small"
            class="core-input"
            @update:value="(value) => updateCoreParam(param.key, value)"
          />
        </div>
        <div class="filter-item reload-form-item">
          <UiButton
            type="primary"
            size="small"
            class="reload-btn"
            :loading="localQuantileLoading"
            @click="reloadPreview"
          >
            <template #icon><img :src="RefreshIcon" alt="" class="reload-icon" /></template>
            重载
          </UiButton>
        </div>
      </div>
    </div>

    <div class="stats-header">

      <div class="stat-item date-range-item">
        <span class="stat-label">数据区间</span>
        <div class="date-picker-wrap">
          <UiDatePicker
            v-model:value="startDate"
            :min="rangeMinMs"
            :max="rangeMaxMs"
            size="small"
            :clearable="false"
            action-text="复位"
            @action="resetStartDate"
          />
          <UiDatePicker
            v-model:value="endDate"
            :min="rangeMinMs"
            :max="rangeMaxMs"
            size="small"
            :clearable="false"
            action-text="复位"
            @action="resetEndDate"
          />
        </div>
      </div>
      <div v-for="stat in stats" :key="stat.label" class="stat-item">
        <span class="stat-label">{{ stat.label }}</span>
        <span class="stat-value">{{ stat.value }}</span>
      </div>
      <div class="stat-item profit-mode-item">
        <span class="stat-label">收益模式</span>
        <UiSelect
          v-model:value="profitMode"
          :options="profitModeOptions"
          size="small"
          class="profit-mode-select"
          :width="300"
        />
      </div>
    </div>

    <FactorMetrics :metrics="factorDetail.metrics" />
    <GroupNavChart
      :datetimes="factorDetail.datetimes"
      :change-percent="factorDetail.changePercent"
      :quantile-names="factorDetail.quantileNames"
      :quantile-count="quantileCount"
      :loading="localQuantileLoading"
      @update:quantile-count="changeQuantileCount"
      @select-date="goDetail"
    />

    <div class="chart-row">
      <IcChart
        :datetimes="factorDetail.datetimes"
        :factor="factorDetail.factor"
        :quantile-names="factorDetail.quantileNames"
      />
      <IndustryIcChart :factor="factorDetail.factor" :quantile-names="factorDetail.quantileNames" />
    </div>

    <div class="chart-row">
      <TurnoverChart
        :datetimes="factorDetail.datetimes"
        :turnover-rate="factorDetail.turnoverRate"
        :quantile-names="factorDetail.quantileNames"
      />
      <DecayChart
        :change-percent="factorDetail.changePercent"
        :quantile-names="factorDetail.quantileNames"
      />
    </div>
  </div>
</template>

<style scoped>
.mode1-preview {
  padding: 24px 32px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.reload-form-item {
  margin-left: auto;
}

.reload-btn {
  min-width: 76px;
  flex-shrink: 0;
}

.reload-icon {
  width: 14px;
  height: 14px;
  filter: brightness(0) invert(1);
}

.filter-bar {
  padding: 16px 20px;
  border-radius: var(--ui-radius-lg);
  background: var(--ui-bg-card);
  box-shadow: var(--ui-shadow-card);
}

.filter-form {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 28px;
}

.filter-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.filter-item__label {
  font-size: var(--ui-font-sm);
  color: var(--ui-text-regular);
  white-space: nowrap;
}

.filter-bar .selector-button {
  min-width: 112px;
  color: var(--ui-color-primary);
}

.filter-bar .core-input {
  width: 112px;
}

.filter-bar .core-input :deep(.ui-input-number__steps) {
  display: none;
}

.stats-header {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
}

.stat-item {
  background: var(--ui-bg-card);
  border-radius: var(--ui-radius-lg);
  padding: 16px 20px;
  box-shadow: var(--ui-shadow-card);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.date-range-item {
  padding: 10px 12px;
}

.date-picker-wrap {
  display: flex;
  flex-direction: row;
  gap: 8px;
}

.date-picker-wrap .ui-datepicker {
  width: 156px;
}

.profit-mode-item {
  min-width: 320px;
}

.profit-mode-select {
  min-width: 280px;
}

.stat-label {
  font-size: 14px;
  color: var(--ui-text-main);
}

.stat-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--ui-text-main);
}

.chart-row {
  display: flex;
  gap: 16px;
}
</style>
