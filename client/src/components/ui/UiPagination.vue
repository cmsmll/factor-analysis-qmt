<script setup lang="ts">
import { computed } from 'vue'

defineOptions({ name: 'UiPagination' })

const props = withDefaults(
  defineProps<{
    page: number
    pageSize: number
    itemCount: number
    pageSizes?: number[]
    showSizePicker?: boolean
    showTotal?: boolean
  }>(),
  { pageSizes: () => [10, 20, 50, 100], showSizePicker: false, showTotal: false },
)

const emit = defineEmits<{
  (e: 'update:page', page: number): void
  (e: 'update:pageSize', size: number): void
}>()

const pageCount = computed(() => Math.max(1, Math.ceil(props.itemCount / props.pageSize)))

/** 展示页码窗口：最多 7 个，带首尾与省略。 */
const visiblePages = computed<Array<number | 'prev-ellipsis' | 'next-ellipsis'>>(() => {
  const total = pageCount.value
  const current = props.page
  if (total <= 7) {
    return Array.from({ length: total }, (_, index) => index + 1)
  }
  const result: Array<number | 'prev-ellipsis' | 'next-ellipsis'> = []
  if (current <= 4) {
    result.push(...[1, 2, 3, 4, 5], 'next-ellipsis', total)
  } else if (current >= total - 3) {
    result.push(1, 'prev-ellipsis', ...[total - 4, total - 3, total - 2, total - 1, total])
  } else {
    result.push(1, 'prev-ellipsis', current - 1, current, current + 1, 'next-ellipsis', total)
  }
  return result
})

function go(target: number): void {
  if (target < 1 || target > pageCount.value || target === props.page) return
  emit('update:page', target)
}

function changeSize(event: Event): void {
  const size = Number((event.target as HTMLSelectElement).value)
  if (Number.isFinite(size) && size > 0) emit('update:pageSize', size)
}
</script>

<template>
  <div class="ui-pagination">
    <span v-if="showTotal" class="ui-pagination__total">共 {{ itemCount }} 条</span>
    <button
      type="button"
      class="ui-pagination__btn"
      :disabled="page <= 1"
      aria-label="上一页"
      @click="go(page - 1)"
    >
      <svg viewBox="0 0 8 12"><path d="M6.5 1L1.5 6l5 5" /></svg>
    </button>

    <template v-for="item in visiblePages" :key="String(item)">
      <button
        v-if="typeof item === 'number'"
        type="button"
        class="ui-pagination__num"
        :class="{ 'is-active': item === page }"
        @click="go(item)"
      >
        {{ item }}
      </button>
      <span v-else class="ui-pagination__ellipsis">…</span>
    </template>

    <button
      type="button"
      class="ui-pagination__btn"
      :disabled="page >= pageCount"
      aria-label="下一页"
      @click="go(page + 1)"
    >
      <svg viewBox="0 0 8 12"><path d="M1.5 1l5 5-5 5" /></svg>
    </button>

    <label v-if="showSizePicker" class="ui-pagination__size">
      <select :value="pageSize" @change="changeSize">
        <option v-for="size in pageSizes" :key="size" :value="size">{{ size }} 条/页</option>
      </select>
    </label>
  </div>
</template>

<style scoped>
.ui-pagination {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--ui-text-regular, #606266);
  font-size: var(--ui-font-sm, 13px);
}

.ui-pagination__total {
  margin-right: 10px;
  color: var(--ui-text-secondary, #909399);
}

.ui-pagination__btn,
.ui-pagination__num {
  display: inline-flex;
  height: 28px;
  min-width: 28px;
  box-sizing: border-box;
  align-items: center;
  justify-content: center;
  padding: 0 6px;
  border: 1px solid var(--ui-border-lighter, #ebeef5);
  border-radius: var(--ui-radius-sm, 4px);
  color: var(--ui-text-regular, #606266);
  background: #fff;
  cursor: pointer;
  font: inherit;
  font-size: var(--ui-font-sm, 13px);
}

.ui-pagination__btn:hover:not(:disabled),
.ui-pagination__num:hover {
  border-color: var(--ui-color-primary, #409eff);
  color: var(--ui-color-primary, #409eff);
}

.ui-pagination__num.is-active {
  border-color: var(--ui-color-primary, #409eff);
  color: #fff;
  background: var(--ui-color-primary, #409eff);
}

.ui-pagination__btn:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.ui-pagination__btn svg {
  width: 8px;
  height: 12px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.ui-pagination__ellipsis {
  min-width: 24px;
  text-align: center;
  color: var(--ui-text-placeholder, #c0c4cc);
}

.ui-pagination__size select {
  height: 28px;
  box-sizing: border-box;
  margin-left: 8px;
  padding: 0 6px;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: var(--ui-radius-sm, 4px);
  outline: none;
  color: var(--ui-text-regular, #606266);
  background: #fff;
  font: inherit;
  font-size: var(--ui-font-xs, 12px);
  cursor: pointer;
}
</style>
