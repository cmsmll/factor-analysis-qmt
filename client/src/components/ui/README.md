# components/ui —— 本地 UI 组件规范

替代原 naive-ui 的自研轻量组件(仅适配中文,不引入第三方 UI 依赖)。三个看板(market/mode1/mode2)与后续新增看板统一使用。

## 组件清单(naive 对应)

| 文件 | 对应 naive | 用法差异 |
|---|---|---|
| UiButton.vue | NButton | `text`→`type="text"`;`secondary`/`quaternary`→`variant`;`#icon` 插槽;文字色随页面 class(`type="text"` 时 color:inherit) |
| UiCard.vue | NCard | `title/size`;header 插槽 `#header` |
| UiTag.vue | NTag | `size="small"`;默认无边框 |
| UiEmpty.vue | NEmpty | `description` |
| UiSpin.vue | NSpin | `:show` 包内容;无内容时独立 spinner |
| UiCheckbox.vue | NCheckbox | `:checked` / `@update:checked` |
| UiRadio.vue / UiRadioGroup.vue | NRadio / NRadioButton / NRadioGroup | Radio `type="button"` 即分段按钮;组用 `UiRadioGroup :value @update:value` |
| UiInput.vue | NInput | `v-model:value`;`clearable` 内建 |
| UiInputNumber.vue | NInputNumber | value 为 `number \| null`,min/max/step |
| UiSelect.vue | NSelect | options `{label,value,disabled?}`;弹层 teleport body;宽度由外层容器控制(根 width:100%) |
| UiPagination.vue | NPagination | `:page`+`@update:page`、`:page-size`+`@update:page-size`;`show-total` 显示"共 N 条" |
| UiDatePicker.vue | NDatePicker | value 毫秒时间戳;`action-text`+`@action` 替代 #now 插槽;面板不自动关闭于 action |
| UiTabs.vue | NTabs/NTabPane | 数据驱动 `:tabs="[{name,label}]"`;内容由页面按活动 tab 渲染单份 |
| UiTable.vue | NDataTable | 列/事件同名:`render(row,index)`、`renderExpand(row)`(单参)、`type:'expand'/'selection'`;`@update:sorter`、`v-model:expanded-row-keys`、`v-model:checked-row-keys`;内部 fixed 布局+sticky th |

## 使用规则

1. 类型:列定义 `import type { UiTableColumn } from '@/components/ui/UiTable.vue'`(替代 `DataTableColumns`)。
2. 文案/格式只做中文;日期 format 仅 `yyyy`/`MM`/`dd` token。
3. 组件状态由 props 受控(`value/checked/page`),页面持状态并监听 `update:*` 事件(与 naive 的 v-model 语义一致)。
4. 自定义交互(弹层定位/展开行/排序)已内建;页面只需透传数据与回调。

## 样式规则

- 主题变量集中在 `src/assets/theme.css`(`:root` 下 `--ui-*` token),由 main.ts 引入。
- 组件专属样式写在各组件 `<style scoped>`;**页面级视觉微调**(padding/颜色/字号覆盖、吸顶)写在页面 SFC scoped,可用 `:deep(.ui-*)` 命中子元素——禁止任何 `:deep(.n-*)`。
- 主题色不得在页面/组件里硬编码:一律 `var(--ui-color-primary)` 等 token;图表专用色与各看板品牌渐变除外(局部语义)。
- 表格吸顶模板:在需要吸顶的页面加
  `.page :deep(.ui-table__th){ position:sticky; top:0; z-index:10; background:var(--ui-bg-header,#fafafa) }`。
- 删除冗余代码:组件卸载/弹层关闭时清理全局监听;不复用则不实现未用能力。
