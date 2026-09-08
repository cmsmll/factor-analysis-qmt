<script setup lang="ts">
import { computed } from 'vue'

defineOptions({ name: 'UiButton' })

const props = withDefaults(
  defineProps<{
    size?: 'tiny' | 'small' | 'medium' | 'large'
    type?: 'default' | 'primary' | 'text'
    /** naive 兼容：secondary=主题色浅底，quaternary=近乎透明 */
    variant?: '' | 'secondary' | 'quaternary'
    circle?: boolean
    block?: boolean
    disabled?: boolean
    loading?: boolean
    /** 覆盖主色（如页面级渐变按钮/文字色） */
    color?: string
  }>(),
  { size: 'medium', type: 'default', variant: '' },
)

const emit = defineEmits<{ (e: 'click', event: MouseEvent): void }>()

const classes = computed(() => ({
  'ui-btn': true,
  [`ui-btn--${props.size}`]: true,
  [`ui-btn--${props.type}`]: true,
  [`ui-btn--${props.variant}`]: props.variant !== '',
  'is-circle': props.circle,
  'is-block': props.block,
  'is-disabled': props.disabled || props.loading,
}))

const style = computed(() =>
  props.color ? { '--ui-btn-color': props.color } : undefined,
)
</script>

<template>
  <button
    type="button"
    :class="classes"
    :style="style"
    :disabled="disabled || loading"
    :aria-disabled="disabled || loading"
    @click="(event: MouseEvent) => emit('click', event)"
  >
    <span v-if="loading" class="ui-btn__spinner" aria-hidden="true"></span>
    <span v-else-if="$slots.icon" class="ui-btn__icon"><slot name="icon" /></span>
    <span v-if="$slots.default" class="ui-btn__label"><slot /></span>
  </button>
</template>

<style scoped>
.ui-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  box-sizing: border-box;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: var(--ui-radius-base, 6px);
  color: var(--ui-text-control, #333234);
  background: transparent;
  cursor: pointer;
  font: inherit;
  white-space: nowrap;
  transition:
    border-color 160ms ease,
    color 160ms ease,
    background-color 160ms ease;
  --ui-btn-color: var(--ui-color-primary, #409eff);
}

.ui-btn:hover {
  border-color: var(--ui-color-primary, #409eff);
  color: var(--ui-color-primary, #409eff);
}

.ui-btn:focus-visible {
  outline: 2px solid var(--ui-color-primary-hover, #66b1ff);
  outline-offset: 1px;
}

.ui-btn.is-disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.ui-btn.is-disabled:hover {
  border-color: var(--ui-border-base, #dcdfe6);
  color: var(--ui-text-regular, #606266);
}

.ui-btn--medium {
  height: 34px;
  padding: 0 15px;
  font-size: var(--ui-font-base, 14px);
}

.ui-btn--small {
  height: 28px;
  padding: 0 11px;
  font-size: 14px;
}

.ui-btn--tiny {
  height: 22px;
  padding: 0 8px;
  font-size: var(--ui-font-xs, 12px);
}

.ui-btn--large {
  height: 40px;
  padding: 0 18px;
  font-size: var(--ui-font-base, 14px);
}

/* 主按钮 */
.ui-btn--primary {
  border-color: var(--ui-btn-color, var(--ui-color-primary, #409eff));
  color: #fff;
  background: var(--ui-btn-color, var(--ui-color-primary, #409eff));
}

.ui-btn--primary:hover {
  border-color: var(--ui-color-primary-hover, #66b1ff);
  color: #fff;
  background: var(--ui-color-primary-hover, #66b1ff);
}

/* 文字按钮：颜色由使用处通过 class/color 控制（inherit 继承） */
.ui-btn--text {
  height: auto;
  padding: 2px 4px;
  border-color: transparent;
  background: transparent;
  color: inherit;
}

.ui-btn--text:hover {
  border-color: transparent;
  color: var(--ui-color-primary, #409eff);
  background: transparent;
}

/* secondary：浅主题色底 */
.ui-btn--secondary {
  border-color: transparent;
  color: var(--ui-color-primary, #409eff);
  background: var(--ui-color-primary-weak, #ecf5ff);
}

.ui-btn--secondary:hover {
  border-color: transparent;
  color: var(--ui-color-primary, #409eff);
  background: color-mix(in srgb, var(--ui-color-primary-weak, #ecf5ff) 80%, #fff);
}

/* quaternary：近透明浅底 */
.ui-btn--quaternary {
  border-color: transparent;
  color: var(--ui-text-regular, #606266);
  background: rgb(0 0 0 / 0.04);
}

.ui-btn--quaternary:hover {
  border-color: transparent;
  color: var(--ui-color-primary, #409eff);
  background: rgb(0 0 0 / 0.07);
}

.is-circle {
  width: 34px;
  padding: 0;
  border-radius: var(--ui-radius-round, 999px);
}

.ui-btn--tiny.is-circle {
  width: 24px;
  height: 24px;
}

.ui-btn--small.is-circle {
  width: 32px;
  height: 32px;
}

.is-block {
  display: flex;
  width: 100%;
}

.ui-btn__icon {
  display: inline-flex;
  align-items: center;
}

.ui-btn__spinner {
  width: 1em;
  height: 1em;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: ui-btn-spin 0.8s linear infinite;
}

@keyframes ui-btn-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
