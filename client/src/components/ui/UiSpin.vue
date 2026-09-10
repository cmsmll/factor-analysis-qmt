<script setup lang="ts">
defineOptions({ name: 'UiSpin' })

withDefaults(
  defineProps<{
    show?: boolean
    size?: number | string
    stroke?: string
  }>(),
  { show: true, size: 24, stroke: '#409eff' },
)
</script>

<template>
  <div class="ui-spin">
    <div v-if="$slots.default" class="ui-spin__content"><slot /></div>
    <div
      v-if="show"
      class="ui-spin__overlay"
      :class="{ 'is-static': !$slots.default }"
      aria-live="polite"
    >
      <span
        class="ui-spin__circle"
        :style="{ width: `${Number(size)}px`, height: `${Number(size)}px`, borderTopColor: stroke }"
      ></span>
    </div>
  </div>
</template>

<style scoped>
.ui-spin {
  position: relative;
  min-height: 0;
}

.ui-spin__overlay {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  pointer-events: none;
}

.ui-spin__overlay.is-static {
  position: static;
  display: grid;
  place-items: center;
}

.ui-spin__circle {
  box-sizing: border-box;
  border: 2px solid rgb(0 0 0 / 0.08);
  border-radius: 50%;
  animation: ui-spin-rotate 0.8s linear infinite;
}

@keyframes ui-spin-rotate {
  to {
    transform: rotate(360deg);
  }
}
</style>
