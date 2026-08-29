<script setup lang="ts">
import { ref } from 'vue'

import KanbanHeader from '@/components/common/KanbanHeader.vue'
import FactorDashboard from '@/views/FactorDashboard.vue'
import Mode2Select from '@/views/Mode2Select.vue'

defineOptions({ name: 'KanbanBoard' })

// 0 = 模式一·分位分析看板，1 = 模式二·因子选股看板
const board = ref(0)

// 各看板标题背景渐变（随看板切换变色）。
const gradients = [
  'linear-gradient(135deg, #1a237e 0%, #283593 40%, #3949ab 100%)',
  'linear-gradient(135deg, #1b5e20 0%, #2e7d32 40%, #43a047 100%)',
]

function switchKanban(step: number) {
  board.value = (board.value + step + 2) % 2
}
</script>

<template>
  <div class="kanban-shell">
    <!-- 页首：同一组件内切换看板（与桌面版 page-header 一致） -->
    <KanbanHeader
      :title="board === 0 ? '因子看盘可视化显示' : '因子选股可视化显示'"
      :subtitle="board === 0 ? '多因子量化分析平台' : '排序 · 过滤 · 截取前 N · 组合回测'"
      :gradient="gradients[board]"
      @prev="switchKanban(-1)"
      @next="switchKanban(1)"
    />
    <div class="kanban-content">
      <FactorDashboard v-show="board === 0" class="kanban-board" />
      <Mode2Select v-show="board === 1" class="kanban-board" />
    </div>
  </div>
</template>

<style scoped>
.kanban-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
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
