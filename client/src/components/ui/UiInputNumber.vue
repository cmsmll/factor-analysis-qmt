<script setup lang="ts">
import { ref, watch } from 'vue'

defineOptions({ name: 'UiInputNumber' })

const props = withDefaults(
  defineProps<{
    value?: number | null
    placeholder?: string
    min?: number
    max?: number
    step?: number
    disabled?: boolean
    size?: 'small' | 'medium'
  }>(),
  { min: -Infinity, max: Infinity, step: 1, size: 'medium' },
)

const emit = defineEmits<{
  (e: 'update:value', value: number | null): void
  (e: 'change', value: number | null): void
}>()

const text = ref(props.value === null || props.value === undefined ? '' : String(props.value))

watch(
  () => props.value,
  (value) => {
    const next = value === null || value === undefined ? '' : String(value)
    if (next !== text.value) text.value = next
  },
)

function commit(): void {
  const trimmed = text.value.trim()
  if (trimmed === '') {
    emit('update:value', null)
    emit('change', null)
    return
  }
  let parsed = Number(trimmed)
  if (Number.isNaN(parsed)) {
    text.value = props.value === null || props.value === undefined ? '' : String(props.value)
    return
  }
  parsed = Math.min(Math.max(parsed, props.min), props.max)
  const value = Number(parsed.toFixed(6))
  text.value = String(value)
  emit('update:value', value)
  emit('change', value)
}

function stepValue(direction: 1 | -1): void {
  const current = props.value ?? 0
  const next = Math.min(Math.max(current + direction * props.step, props.min), props.max)
  text.value = String(next)
  emit('update:value', next)
  emit('change', next)
}
</script>

<template>
  <span class="ui-input-number" :class="[`ui-input-number--${size}`, { 'is-disabled': disabled }]">
    <input
      v-model="text"
      type="text"
      inputmode="decimal"
      :placeholder="placeholder"
      :disabled="disabled"
      class="ui-input-number__control"
      @change="commit"
      @keydown.enter.prevent="commit"
      @keydown.up.prevent="stepValue(1)"
      @keydown.down.prevent="stepValue(-1)"
      @blur="commit"
    />
    <span class="ui-input-number__steps">
      <button type="button" tabindex="-1" aria-label="增加" :disabled="disabled" @click="stepValue(1)">
        <svg viewBox="0 0 8 8"><path d="M1 5.5L4 2.5l3 3" /></svg>
      </button>
      <button type="button" tabindex="-1" aria-label="减少" :disabled="disabled" @click="stepValue(-1)">
        <svg viewBox="0 0 8 8"><path d="M1 2.5L4 5.5l3-3" /></svg>
      </button>
    </span>
  </span>
</template>

<style scoped>
.ui-input-number {
  display: inline-flex;
  align-items: stretch;
  box-sizing: border-box;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: var(--ui-radius-base, 6px);
  background: #fff;
  overflow: hidden;
}

.ui-input-number.is-disabled {
  opacity: 0.75;
  background: var(--ui-bg-header, #fafafa);
}

.ui-input-number:focus-within {
  border-color: var(--ui-color-primary, #409eff);
}

.ui-input-number__control {
  min-width: 0;
  box-sizing: border-box;
  border: 0;
  outline: 0;
  padding: 0 8px;
  color: var(--ui-text-control, #333234);
  background: transparent;
  font-size: var(--ui-font-xs, 12px);
}

.ui-input-number--medium .ui-input-number__control {
  height: 32px;
  font-size: var(--ui-font-base, 14px);
}

.ui-input-number__steps {
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--ui-border-extra-light, #f2f3f5);
}

.ui-input-number__steps button {
  display: grid;
  width: 20px;
  height: 50%;
  place-items: center;
  box-sizing: border-box;
  border: 0;
  border-bottom: 1px solid var(--ui-border-extra-light, #f2f3f5);
  color: var(--ui-text-secondary, #909399);
  background: transparent;
  cursor: pointer;
  padding: 0;
}

.ui-input-number__steps button:last-child {
  border-bottom: 0;
}

.ui-input-number__steps button:hover {
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-bg-hover, #f5f7fa);
}

.ui-input-number__steps svg {
  width: 8px;
  height: 8px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>
