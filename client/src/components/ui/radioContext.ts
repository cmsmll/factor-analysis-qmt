import type { ComputedRef } from 'vue'

/** UiRadio / UiRadioGroup 共享注入契约。 */
export interface UiRadioContext {
  value: ComputedRef<string | number | boolean>
  disabled: ComputedRef<boolean>
  onChange: (value: string | number | boolean) => void
}

export const RADIO_GROUP_KEY = Symbol('ui-radio-group')
