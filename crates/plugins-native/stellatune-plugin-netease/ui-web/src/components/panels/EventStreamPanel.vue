<script setup lang="ts">
import { computed, ref } from "vue";
import UiSelect from "../UiSelect.vue";
import type { UiEventRow } from "../../view-models";

defineOptions({
  name: "EventStreamPanel"
});

const props = defineProps<{
  events: UiEventRow[];
  onClearEvents: () => unknown;
}>();

const eventKeyword = ref("");
const eventNameFilter = ref("全部");
const eventsExpanded = ref(false);

const eventNameOptions = computed(() => {
  const names = new Set<string>();
  for (const item of props.events) {
    names.add(item.name);
  }
  return ["全部", ...Array.from(names.values())];
});
const filteredEvents = computed(() => {
  const keyword = eventKeyword.value.trim().toLowerCase();
  return props.events.filter((item) => {
    if (eventNameFilter.value !== "全部" && item.name !== eventNameFilter.value) {
      return false;
    }
    if (!keyword) {
      return true;
    }
    return item.searchable.includes(keyword);
  });
});
const eventCountLabel = computed(
  () => `共 ${props.events.length} 条事件，筛选后 ${filteredEvents.value.length} 条`
);
const eventToggleLabel = computed(() => (eventsExpanded.value ? "折叠全部原始数据" : "展开全部原始数据"));

function toggleExpandEvents(): void {
  eventsExpanded.value = !eventsExpanded.value;
}
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2>事件流</h2>
      <div class="actions">
        <button :disabled="events.length === 0" @click="toggleExpandEvents">{{ eventToggleLabel }}</button>
        <button :disabled="events.length === 0" @click="onClearEvents()">清空事件</button>
      </div>
    </div>
    <div class="event-toolbar">
      <input
        v-model="eventKeyword"
        type="text"
        placeholder="输入关键词过滤（事件名、摘要、原始数据）"
      />
      <UiSelect v-model="eventNameFilter" :options="eventNameOptions" />
    </div>
    <p class="hint">{{ eventCountLabel }}</p>
    <ul class="event-list">
      <li v-for="item in filteredEvents" :key="item.id">
        <div class="event-head-row">
          <span class="event-time">{{ item.time }}</span>
          <strong class="event-name">{{ item.name }}</strong>
          <span class="event-source">{{ item.source }}</span>
        </div>
        <p class="event-summary">{{ item.summary }}</p>
        <details class="raw-panel" :open="eventsExpanded">
          <summary>查看原始数据</summary>
          <pre class="payload">{{ item.raw }}</pre>
        </details>
      </li>
      <li v-if="filteredEvents.length === 0" class="event-empty">当前筛选条件下没有事件。</li>
    </ul>
  </section>
</template>
