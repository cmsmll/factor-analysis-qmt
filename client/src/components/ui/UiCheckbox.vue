<script setup lang="ts">
defineOptions({ name: 'UiCheckbox' })

const props = withDefaults(
  defineProps<{
    checked?: boolean
    disabled?: boolean
    label?: string
    /** 语义色覆盖（默认主色） */
    color?: string
  }>(),
  { color: '' },
)

const emit = defineEmits<{
  (e: 'update:checked', value: boolean): void
  (e: 'change', value: boolean): void
}>()

function toggle(): void {
  emit('update:checked', !props.checked)
  emit('change', !props.checked)
}
</script>

<template>
  <label class="ui-checkbox" :class="{ 'is-disabled': disabled }">
    <input
      class="ui-checkbox__input"
      type="checkbox"
      :checked="checked"
      :disabled="disabled"
      @change="toggle"
    />
    <span class="ui-checkbox__box" :style="color ? { '--ui-checkbox-color': color } : undefined">
      <svg viewBox="0 0 12 10" aria-hidden="true"><path d="M1.5 5l3 3 6-7" /></svg>
    </span>
    <span v-if="label || $slots.default" class="ui-checkbox__label">
      <slot>{{ label }}</slot>
    </span>
  </label>
</template>

<style scoped>
.ui-checkbox {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  --ui-checkbox-color: var(--ui-color-primary, #409eff);
  user-select: none;
}

.ui-checkbox.is-disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.ui-checkbox__input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

.ui-checkbox__box {
  display: inline-flex;
  width: 15px;
  height: 15px;
  flex: 0 0 15px;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  border: 1px solid var(--ui-border-base, #dcdfe6);
  border-radius: 3px;
  color: #fff;
  background: #fff;
  transition:
    border-color 160ms ease,
    background-color 160ms ease;
}

.ui-checkbox__box svg {
  width: 11px;
  height: 9px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
  opacity: 0;
}

.ui-checkbox__input:checked + .ui-checkbox__box {
  border-color: var(--ui-checkbox-color, var(--ui-color-primary, #409eff));
  background: var(--ui-checkbox-color, var(--ui-color-primary, #409eff));
}

.ui-checkbox__input:checked + .ui-checkbox__box svg {
  opacity: 1;
}

.ui-checkbox__input:focus-visible + .ui-checkbox__box {
  outline: 2px solid var(--ui-color-primary-hover, #66b1ff);
  outline-offset: 1px;
}

.ui-checkbox__label {
  color: var(--ui-text-regular, #606266);
  font-size: var(--ui-font-base, 14px);
}
</style>
