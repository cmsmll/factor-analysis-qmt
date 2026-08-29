<script setup lang="ts">
import { NButton } from 'naive-ui'

defineOptions({ name: 'KanbanHeader' })

defineProps<{
  title: string
  subtitle: string
  /** 上一看板不可达（首块看板） */
  prevDisabled?: boolean
  /** 下一看板不可达（末块看板） */
  nextDisabled?: boolean
}>()

const emit = defineEmits<{ (e: 'prev'): void; (e: 'next'): void }>()
</script>

<template>
  <header class="page-header">
    <NButton
      text
      circle
      class="header-switch-btn"
      :disabled="prevDisabled"
      aria-label="上一看板"
      @click="emit('prev')"
    >
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
    </NButton>
    <div class="header-content">
      <h1 class="page-title">{{ title }}</h1>
      <p class="page-subtitle">{{ subtitle }}</p>
    </div>
    <NButton
      text
      circle
      class="header-switch-btn"
      :disabled="nextDisabled"
      aria-label="下一看板"
      @click="emit('next')"
    >
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
    </NButton>
  </header>
</template>

<style scoped>
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: linear-gradient(135deg, #1a237e 0%, #283593 40%, #3949ab 100%);
  border-radius: 10px;
  box-shadow: 0 4px 20px rgba(26, 35, 126, 0.25);
  overflow: hidden;
  padding: 28px 32px;
}

.header-switch-btn {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  color: rgba(255, 255, 255, 0.88);
  background: rgba(255, 255, 255, 0.1);
}

.header-switch-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.18);
}

.header-switch-btn svg {
  width: 22px;
  height: 22px;
}

.header-content {
  flex: 1;
  text-align: center;
  position: relative;
}

.header-content::after {
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

.header-content::before {
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

.page-title {
  margin: 0;
  font-size: 30px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 2px;
  position: relative;
}

.page-subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.65);
  letter-spacing: 4px;
}
</style>
