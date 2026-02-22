<script setup lang="ts">
import { computed } from "vue";

defineOptions({
  name: "JsonTreeNode"
});

const props = defineProps<{
  nodeKey: string;
  path: string;
  value: unknown;
  depth: number;
}>();

interface TreeChild {
  key: string;
  path: string;
  value: unknown;
}

const nodeType = computed(() => detectType(props.value));
const previewText = computed(() => preview(props.value));
const children = computed<TreeChild[]>(() => readChildren(props.value, props.path));
const expandable = computed(() => children.value.length > 0);
const childCount = computed(() => children.value.length);
const defaultOpen = computed(() => props.depth < 1);

function readChildren(value: unknown, path: string): TreeChild[] {
  if (Array.isArray(value)) {
    return value.map((item, index) => ({
      key: `[${index}]`,
      path: `${path}[${index}]`,
      value: item
    }));
  }
  if (typeof value !== "object" || value === null) {
    return [];
  }
  return Object.entries(value as Record<string, unknown>).map(([key, childValue]) => ({
    key,
    path: `${path}${formatPathSuffix(key)}`,
    value: childValue
  }));
}

function formatPathSuffix(key: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    return `.${key}`;
  }
  return `[${JSON.stringify(key)}]`;
}

function detectType(value: unknown): string {
  if (value === null) {
    return "空值";
  }
  if (Array.isArray(value)) {
    return "数组";
  }
  if (typeof value === "string") {
    return "字符串";
  }
  if (typeof value === "number") {
    return "数字";
  }
  if (typeof value === "boolean") {
    return "布尔";
  }
  if (typeof value === "object") {
    return "对象";
  }
  return typeof value;
}

function preview(value: unknown): string {
  if (value === null) {
    return "null";
  }
  if (typeof value === "string") {
    const oneLine = value.replace(/\s+/g, " ");
    return oneLine.length <= 120 ? oneLine : `${oneLine.slice(0, 120)}...`;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return `数组（${value.length} 项）`;
  }
  if (typeof value === "object") {
    const size = Object.keys(value as Record<string, unknown>).length;
    return `对象（${size} 字段）`;
  }
  return String(value);
}
</script>

<template>
  <li class="json-tree-node">
    <details v-if="expandable" :open="defaultOpen" class="json-tree-branch">
      <summary class="json-tree-summary">
        <span class="json-tree-toggle" aria-hidden="true"></span>
        <code class="json-tree-key">{{ nodeKey }}</code>
        <span class="json-tree-type">{{ nodeType }}</span>
        <span class="json-tree-preview">{{ previewText }}</span>
        <span class="json-tree-count">{{ childCount }} 项</span>
      </summary>
      <ul class="json-tree-children">
        <JsonTreeNode
          v-for="child in children"
          :key="child.path"
          :node-key="child.key"
          :path="child.path"
          :value="child.value"
          :depth="depth + 1"
        />
      </ul>
    </details>
    <div v-else class="json-tree-leaf">
      <span class="json-tree-dot" aria-hidden="true"></span>
      <code class="json-tree-key">{{ nodeKey }}</code>
      <span class="json-tree-type">{{ nodeType }}</span>
      <span class="json-tree-preview">{{ previewText }}</span>
    </div>
  </li>
</template>
