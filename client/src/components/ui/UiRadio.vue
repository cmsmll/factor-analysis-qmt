<script setup lang="ts">
import { inject } from 'vue'
import { RADIO_GROUP_KEY, type UiRadioContext } from './radioContext'

defineOptions({ name: 'UiRadio' })

const props = withDefaults(
  defineProps<{
    value: string | number | boolean
    disabled?: boolean
    /** button 变体：分段按钮观感（对应 naive NRadioButton） */
    type?: 'default' | 'button'
  }>(),
  { type: 'default' },
)

const ctx = inject<UiRadioContext | null>(RADIO_GROUP_KEY, null)

function select(): void {
  if (props.disabled || ctx?.disabled.value) return
  ctx?.onChange(props.value)
}

const active = () => ctx?.value.value === props.value
</script>

<template>
  <label
    class="ui-radio"
    :class="[
      ctx ? `ui-radio--${type}` : 'ui-radio--default',
      { 'is-active': active(), 'is-disabled': disabled || ctx?.disabled.value },
    ]"
    role="radio"
    :aria-checked="active()"
    tabindex="0"
    @keydown.enter.prevent="select"
    @keydown.space.prevent="select"
  >
    <input
      class="ui-radio__native"
      type="radio"
      :checked="active()"
      :disabled="disabled || ctx?.disabled.value"
      @change="select"
    />
    <template v-if="type === 'button' || !ctx">
      <span class="ui-radio__segment"><slot /></span>
    </template>
    <template v-else>
      <span class="ui-radio__dot" aria-hidden="true"></span>
      <span class="ui-radio__label"><slot /></span>
    </template>
  </label>
</template>

<style scoped>
.ui-radio {
  display: inline-flex;
  align-items: center;
  cursor: pointer;
  user-select: none;
}

.ui-radio.is-disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.ui-radio__native {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

/* 圆点样式（默认变体） */
.ui-radio--default {
  gap: 6px;
  color: var(--ui-text-regular, #606266);
  font-size: var(--ui-font-sm, 13px);
}

.ui-radio__dot {
  position: relative;
  width: 15px;
  height: 15px;
  box-sizing: border-box;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: 50%;
  background: #fff;
  transition: border-color 160ms ease;
}

.ui-radio--default.is-active .ui-radio__dot {
  border-color: var(--ui-color-primary, #409eff);
}

.ui-radio__dot::after {
  position: absolute;
  inset: 3px;
  border-radius: 50%;
  background: var(--ui-color-primary, #409eff);
  content: '';
  opacity: 0;
  transform: scale(0.4);
  transition:
    opacity 160ms ease,
    transform 160ms ease;
}

.ui-radio--default.is-active .ui-radio__dot::after {
  opacity: 1;
  transform: scale(1);
}

.ui-radio--default.is-active {
  color: var(--ui-color-primary, #409eff);
}

/* 分段按钮变体（NRadioButton） */
.ui-radio--button {
  margin: 0;
}

.ui-radio__segment {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 14px;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  margin-left: -1px;
  color: var(--ui-text-regular, #606266);
  font-size: var(--ui-font-sm, 13px);
  white-space: nowrap;
}

.ui-radio--button:first-of-type .ui-radio__segment {
  border-radius: 6px 0 0 6px;
  margin-left: 0;
}

.ui-radio--button:last-of-type .ui-radio__segment {
  border-radius: 0 6px 6px 0;
}

.ui-radio--button.is-active .ui-radio__segment {
  position: relative;
  z-index: 1;
  border-color: var(--ui-color-primary, #409eff);
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary-weak, #ecf5ff);
}
</style>
