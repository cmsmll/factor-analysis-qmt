<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import { boardStepPath } from '@/utils/boardNav'
import UiButton from '@/components/ui/UiButton.vue'
import BoardFilterBar from '@/components/board/BoardFilterBar.vue'

withDefaults(
  defineProps<{
    title: string
    subtitle?: string
    /** 头部渐变背景 */
    gradient: string
    /** 头部阴影(可选) */
    shadow?: string
    /** 页脚文案 */
    footer?: string
  }>(),
  { subtitle: '', shadow: '', footer: '' },
)

const route = useRoute()
const router = useRouter()

function switchBoard(step: number): void {
  void router.push(boardStepPath(route.path, step))
}
</script>

<template>
  <div class="board-shell">
    <header
      class="board-header"
      :style="{ background: gradient, boxShadow: shadow || undefined }"
    >
      <UiButton type="text" class="board-switch-btn" aria-label="上一看板" @click="switchBoard(-1)">
        <template #icon>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
            <path
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="48"
              d="M328 112L184 256l144 144"
            ></path>
          </svg>
        </template>
      </UiButton>
      <div class="board-header-content">
        <h1 class="board-title">{{ title }}</h1>
        <p v-if="subtitle" class="board-subtitle">{{ subtitle }}</p>
      </div>
      <UiButton type="text" class="board-switch-btn" aria-label="下一看板" @click="switchBoard(1)">
        <template #icon>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
            <path
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="48"
              d="M184 112l144 144-144 144"
            ></path>
          </svg>
        </template>
      </UiButton>
    </header>

    <BoardFilterBar>
      <slot name="filters" />
    </BoardFilterBar>

    <slot />

    <footer v-if="footer" class="board-footer">
      <span>{{ footer }}</span>
    </footer>
  </div>
</template>

<style scoped>
.board-shell {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 32px;
  max-width: 1440px;
  margin: 0 auto;
}

.board-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-radius: 10px;
  overflow: hidden;
  padding: 28px 32px;
}

.board-switch-btn {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.88);
  background: rgba(255, 255, 255, 0.1);
}

.board-switch-btn:hover {
  border-color: transparent;
  color: #fff;
  background: rgba(255, 255, 255, 0.18);
}

.board-switch-btn svg {
  width: 22px;
  height: 22px;
}

.board-header-content {
  flex: 1;
  text-align: center;
  position: relative;
}

.board-header-content::after {
  content: '';
  position: absolute;
  left: -80px;
  top: -50px;
  width: 140px;
  height: 140px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.04);
  pointer-events: none;
}

.board-header-content::before {
  content: '';
  position: absolute;
  right: -60px;
  bottom: -40px;
  width: 100px;
  height: 100px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.03);
  pointer-events: none;
}

.board-title {
  margin: 0;
  font-size: 30px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 2px;
  position: relative;
}

.board-subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.65);
  letter-spacing: 4px;
}

.board-footer {
  display: flex;
  justify-content: center;
  padding: 12px 0;
  color: var(--ui-text-secondary, #909399);
  font-size: 13px;
}
</style>
