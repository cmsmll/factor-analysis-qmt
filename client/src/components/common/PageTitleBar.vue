<script setup lang="ts">
import { NButton } from 'naive-ui'

defineOptions({ name: 'PageTitleBar' })

withDefaults(
  defineProps<{
    /** 标题文本 */
    title: string
    /** 标题悬浮提示 */
    titleTip?: string
    /** 是否显示「明细」按钮（默认显示） */
    showDetail?: boolean
  }>(),
  { showDetail: true },
)

const emit = defineEmits<{ (e: 'back'): void; (e: 'detail'): void }>()
</script>

<template>
  <div class="page-title-bar">
    <NButton text size="small" class="back-btn" aria-label="返回" @click="emit('back')">
      <template #icon>
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" class="back-icon">
          <path
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="48"
            d="M244 400L100 256l144-144"
          ></path>
          <path
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="48"
            d="M120 256h292"
          ></path>
        </svg>
      </template>
      返回
    </NButton>
    <h2 class="factor-title" :title="titleTip">{{ title }}</h2>
    <NButton v-if="showDetail" text size="small" class="detail-btn" @click="emit('detail')">
      明细
    </NButton>
  </div>
</template>

<style scoped>
.page-title-bar {
  display: flex;
  align-items: center;
  background: #fff;
  padding: 14px 24px;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  gap: 16px;
}

.back-btn {
  flex-shrink: 0;
  color: var(--n-text-color);
  font-size: 13px;
}

.back-icon {
  width: 16px;
  height: 16px;
}

.factor-title {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  color: #1a237e;
  flex: 1;
  text-align: center;
}

.detail-btn {
  flex-shrink: 0;
  color: #409eff;
}
</style>
