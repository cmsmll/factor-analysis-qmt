<script setup lang="ts">
withDefaults(defineProps<{ description?: string }>(), { description: '' })
</script>

<template>
  <div class="ui-empty">
    <svg class="ui-empty__icon" viewBox="0 0 64 64" aria-hidden="true">
      <circle cx="32" cy="32" r="26" fill="none" stroke="currentColor" stroke-width="2" opacity="0.3" />
      <path
        d="M18 44c4-6 8-9 14-9s10 3 14 9"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        opacity="0.3"
      />
      <circle cx="24" cy="26" r="3" fill="currentColor" opacity="0.4" />
      <circle cx="40" cy="26" r="3" fill="currentColor" opacity="0.4" />
    </svg>
    <p v-if="description" class="ui-empty__desc">{{ description }}</p>
    <slot />
  </div>
</template>

<style scoped>
.ui-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px 0;
  color: var(--ui-text-placeholder, #c0c4cc);
}

.ui-empty__icon {
  width: 64px;
  height: 64px;
  color: var(--ui-text-placeholder, #c0c4cc);
}

.ui-empty__desc {
  margin: 0;
  font-size: var(--ui-font-sm, 13px);
  color: var(--ui-text-secondary, #909399);
}
</style>
