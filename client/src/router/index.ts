import { createRouter, createWebHistory } from 'vue-router'

import KanbanBoard from '@/views/KanbanBoard.vue'
import Mode1Refine from '@/views/Mode1Refine.vue'
import Mode1Preview from '@/views/Mode1Preview.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      redirect: '/mode1',
    },
    {
      path: '/mode1',
      name: 'mode1',
      component: KanbanBoard,
    },
    {
      path: '/mode2',
      name: 'mode2',
      component: KanbanBoard,
    },
    {
      path: '/mode2/:id',
      name: 'mode2-preview',
      component: KanbanBoard,
    },
    {
      path: '/mode1/:id',
      name: 'mode1-preview',
      component: Mode1Preview,
    },
    {
      path: '/mode1/:id/refine',
      name: 'mode1-refine',
      component: Mode1Refine,
    },
  ],
})

export default router
