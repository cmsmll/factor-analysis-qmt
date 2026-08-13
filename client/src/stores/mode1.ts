import { ref, shallowRef } from "vue";
import { defineStore } from "pinia";

import { fetchMode1List, fetchPeriods } from "@/api/mode1";
import type { ModeFilter, ModeListItem, ModeRequest, Period } from "@/types/mode1";

const FILTER_CACHE_KEY = "mode1-filter";
const PREVIEW_PARAMS_PREFIX = "mode1-preview-params-";

export function loadCachedFilter(): ModeFilter | null {
  try {
    const raw = localStorage.getItem(FILTER_CACHE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as ModeFilter;
  } catch {
    return null;
  }
}

export function savePreviewParams(id: string, params: ModeRequest): void {
  try {
    localStorage.setItem(PREVIEW_PARAMS_PREFIX + id, JSON.stringify(params));
  } catch {
    // localStorage full or unavailable
  }
}

export function loadPreviewParams(id: string): ModeRequest | null {
  try {
    const raw = localStorage.getItem(PREVIEW_PARAMS_PREFIX + id);
    if (!raw) return null;
    return JSON.parse(raw) as ModeRequest;
  } catch {
    return null;
  }
}
export const useMode1Store = defineStore("mode1", () => {
  const periods = shallowRef<Period[]>([]);
  const items = shallowRef<ModeListItem[]>([]);
  const curr = shallowRef<ModeListItem | null>(null);
  const periodLoading = ref(false);
  const listLoading = ref(false);
  const periodError = ref("");
  const listError = ref("");
  let listRequestVersion = 0;

  async function loadPeriods(force = false): Promise<void> {
    if (periodLoading.value || (periods.value.length > 0 && !force)) return;

    periodLoading.value = true;
    periodError.value = "";
    try {
      periods.value = await fetchPeriods();
      if (periods.value.length === 0) throw new Error("没有可用的时间周期配置");
    } catch (error) {
      periodError.value =
        error instanceof Error ? error.message : "获取时间周期失败";
    } finally {
      periodLoading.value = false;
    }
  }

  async function loadList(filter: ModeFilter): Promise<void> {
    const version = ++listRequestVersion;
    listLoading.value = true;
    listError.value = "";

    try {
      const data = await fetchMode1List(filter);
      if (listRequestVersion === version) {
        items.value = sortModeListItems(data);
        try { localStorage.setItem(FILTER_CACHE_KEY, JSON.stringify(filter)); } catch { /* ignore */ }
      }
    } catch (error) {
      if (listRequestVersion === version) {
        listError.value =
          error instanceof Error ? error.message : "获取模式一列表失败";
      }
    } finally {
      if (listRequestVersion === version) listLoading.value = false;
    }
  }

  async function loadDefaultList(): Promise<void> {
    if (items.value.length > 0) return;

    await loadPeriods();
    const period = periods.value[0];
    if (!period || periodError.value) return;
    await loadList(createModeFilter(period));
  }

  return {
    periods,
    items,
    curr,
    periodLoading,
    listLoading,
    periodError,
    listError,
    loadPeriods,
    loadList,
    loadDefaultList,
    setCurr: (item: ModeListItem | null) => {
      curr.value = item;
    },
  };
});

export function createModeFilter(period: Period): ModeFilter {
  return {
    start: period.start,
    end: period.end,
    filter_bz: false,
    filter_st: false,
    sector: [],
    indice: [],
  };
}

function sortModeListItems(data: ModeListItem[]): ModeListItem[] {
  return [...data].sort((left, right) => {
    const result = factorName(left).localeCompare(factorName(right), "zh-CN", {
      numeric: true,
      sensitivity: "base",
    });
    return result || left.args.base.id.localeCompare(right.args.base.id);
  });
}

function factorName(item: ModeListItem): string {
  return item.data?.name || item.args.base.id;
}
