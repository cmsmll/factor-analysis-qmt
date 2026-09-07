import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      redirect: '/market',
    },
    {
      path: '/market',
      name: 'market',
      component: () => import('@/views/MarketBoard.vue'),
    },
    {
      path: '/market/:code',
      name: 'market-kline',
      component: () => import('@/views/MarketKLine.vue'),
    },
    {
      path: '/mode1',
      name: 'mode1',
      component: () => import('@/views/KanbanBoard.vue'),
    },
    {
      path: '/mode2',
      name: 'mode2',
      component: () => import('@/views/KanbanBoard.vue'),
    },
    {
      path: '/mode2/:id',
      name: 'mode2-preview',
      component: () => import('@/views/Mode2Preview.vue'),
    },
    {
      path: '/mode2/:id/detail',
      name: 'mode2-detail',
      component: () => import('@/views/Mode2Detail.vue'),
    },
    {
      path: '/mode1/:id',
      name: 'mode1-preview',
      component: () => import('@/views/Mode1Preview.vue'),
    },
    {
      path: '/mode1/:id/detail',
      name: 'mode1-detail',
      component: () => import('@/views/Mode1Detail.vue'),
    },
  ],
})

export default router
