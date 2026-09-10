<script setup lang="ts">
import { ref, watch } from 'vue'

defineOptions({ name: 'UiInput' })

const props = withDefaults(
  defineProps<{
    /** naive 兼容：v-model:value 绑定 */
    value?: string
    type?: string
    placeholder?: string
    clearable?: boolean
    disabled?: boolean
    size?: 'small' | 'medium' | 'large'
    readonly?: boolean
    autofocus?: boolean
  }>(),
  { type: 'text', size: 'medium' },
)

const emit = defineEmits<{
  (e: 'update:value', value: string): void
  (e: 'change', value: string): void
  (e: 'focus', event: FocusEvent): void
  (e: 'blur', event: FocusEvent): void
  (e: 'keydown', event: KeyboardEvent): void
  (e: 'clear'): void
}>()

const inner = ref(props.value ?? '')

watch(
  () => props.value,
  (value) => {
    if (value !== inner.value) inner.value = value ?? ''
  },
)

function onInput(event: Event): void {
  inner.value = (event.target as HTMLInputElement).value
  emit('update:value', inner.value)
}

function onBlur(event: FocusEvent): void {
  emit('blur', event)
  emit('change', inner.value)
}

function clear(): void {
  inner.value = ''
  emit('update:value', '')
  emit('clear')
  emit('change', '')
}
</script>

<template>
  <span class="ui-input" :class="[`ui-input--${size}`, { 'is-disabled': disabled }]">
    <slot name="prefix" />
    <input
      v-model="inner"
      :type="type"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readonly"
      :autofocus="autofocus"
      class="ui-input__control"
      @input="onInput"
      @blur="onBlur"
      @focus="$emit('focus', $event)"
      @keydown="$emit('keydown', $event)"
    />
    <button
      v-if="clearable && inner && !disabled"
      type="button"
      class="ui-input__clear"
      aria-label="清空"
      tabindex="-1"
      @mousedown.prevent
      @click="clear"
    >
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 2l8 8M10 2l-8 8" /></svg>
    </button>
    <slot name="suffix" />
  </span>
</template>

<style scoped>
.ui-input {
  display: inline-flex;
  align-items: center;
  box-sizing: border-box;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: var(--ui-radius-base, 6px);
  background: #fff;
  color: var(--ui-text-control, #333234);
  transition:
    border-color 160ms ease,
    box-shadow 160ms ease;
}

.ui-input:focus-within {
  border-color: var(--ui-color-primary, #409eff);
  box-shadow: 0 0 0 2px rgb(64 158 255 / 0.18);
}

.ui-input.is-disabled {
  cursor: not-allowed;
  opacity: 0.75;
  background: var(--ui-bg-header, #fafafa);
}

.ui-input--small {
  height: 28px;
}

.ui-input--medium {
  height: 34px;
}

.ui-input--large {
  height: 40px;
}

.ui-input__control {
  min-width: 0;
  flex: 1;
  height: 100%;
  box-sizing: border-box;
  border: 0;
  outline: 0;
  padding: 0 10px;
  color: inherit;
  background: transparent;
  font: inherit;
  font-size: var(--ui-font-base, 14px);
}

.ui-input--small .ui-input__control {
  font-size: var(--ui-font-xs, 12px);
}

.ui-input__control::placeholder {
  color: var(--ui-text-placeholder, #c0c4cc);
}

.ui-input__clear {
  display: inline-flex;
  width: 18px;
  height: 18px;
  margin-right: 6px;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  border: 0;
  border-radius: 50%;
  color: var(--ui-text-placeholder, #c0c4cc);
  background: rgb(0 0 0 / 0.06);
  cursor: pointer;
}

.ui-input__clear:hover {
  color: var(--ui-text-regular, #606266);
}

.ui-input__clear svg {
  width: 9px;
  height: 9px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
}
</style>
