<script setup lang="ts">
import { RouterView } from 'vue-router'

import GlobalFilterSelector from '@/components/common/GlobalFilterSelector.vue'
import GlobalLoading from '@/components/common/GlobalLoading.vue'
import GlobalMessage from '@/components/common/GlobalMessage.vue'
</script>

<template>
  <RouterView v-slot="{ Component, route }">
    <KeepAlive include="KanbanBoard">
      <component
        :is="Component"
        v-if="route.name === 'mode1' || route.name === 'mode2'"
        :key="route.name"
      />
    </KeepAlive>
    <component
      :is="Component"
      v-if="route.name !== 'mode1' && route.name !== 'mode2'"
      :key="route.fullPath"
    />
  </RouterView>
  <GlobalFilterSelector />
  <GlobalLoading />
  <GlobalMessage />
</template>
