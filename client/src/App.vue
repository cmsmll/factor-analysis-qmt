<script setup lang="ts">
import { RouterLink, RouterView, useRoute } from 'vue-router'

import GlobalFilterSelector from '@/components/common/GlobalFilterSelector.vue'
import GlobalLoading from '@/components/common/GlobalLoading.vue'
import GlobalMessage from '@/components/common/GlobalMessage.vue'

const route = useRoute()
</script>

<template>
  <header class="app-nav">
    <RouterLink to="/mode1" class="nav-link" :class="{ active: route.path.startsWith('/mode1') }">
      模式一 · 分位分析
    </RouterLink>
    <RouterLink to="/mode2" class="nav-link" :class="{ active: route.path.startsWith('/mode2') }">
      模式二 · 因子选股
    </RouterLink>
  </header>
  <RouterView v-slot="{ Component, route }">
    <KeepAlive include="FactorDashboard">
      <component :is="Component" v-if="route.name === 'mode1'" />
    </KeepAlive>
    <component :is="Component" v-if="route.name !== 'mode1'" :key="route.fullPath" />
  </RouterView>
  <GlobalFilterSelector />
  <GlobalLoading />
  <GlobalMessage />
</template>

<style scoped>
.app-nav {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  border-bottom: 1px solid #e4e7ed;
  background: #fff;
  position: sticky;
  top: 0;
  z-index: 10;
}

.nav-link {
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 14px;
  color: #606266;
  text-decoration: none;
}

.nav-link:hover {
  color: #409eff;
  background: #ecf5ff;
}

.nav-link.active {
  color: #409eff;
  font-weight: 600;
  background: #ecf5ff;
}
</style>
