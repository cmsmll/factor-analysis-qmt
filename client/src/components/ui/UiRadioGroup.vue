<script setup lang="ts">
import { computed, provide } from 'vue'
import { RADIO_GROUP_KEY, type UiRadioContext } from './radioContext'

defineOptions({ name: 'UiRadioGroup' })

const props = withDefaults(
  defineProps<{
    value: string | number | boolean
    disabled?: boolean
    size?: 'small' | 'medium'
  }>(),
  { size: 'medium' },
)

const emit = defineEmits<{ (e: 'update:value', value: string | number | boolean): void }>()

const context: UiRadioContext = {
  value: computed(() => props.value),
  disabled: computed(() => props.disabled),
  onChange: (next) => {
    if (next === props.value) return
    emit('update:value', next)
  },
}
provide(RADIO_GROUP_KEY, context)
</script>

<template>
  <div class="ui-radio-group" :class="`ui-radio-group--${size}`" role="radiogroup">
    <slot />
  </div>
</template>

<style scoped>
.ui-radio-group {
  display: inline-flex;
  align-items: center;
}

.ui-radio-group--small :deep(.ui-radio__segment) {
  height: 24px;
  padding: 0 10px;
  font-size: var(--ui-font-xs, 12px);
}
</style>
