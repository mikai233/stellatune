<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

defineOptions({
  name: "UiSelect"
});

const props = defineProps<{
  modelValue: string;
  options: Array<string | { value: string; label: string }>;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

const rootRef = ref<HTMLElement | null>(null);
const isOpen = ref(false);

const normalizedOptions = computed(() =>
  props.options.map((option) => {
    if (typeof option === "string") {
      return {
        value: option,
        label: option
      };
    }
    return option;
  })
);

const selectedLabel = computed(() => {
  const matched = normalizedOptions.value.find((item) => item.value === props.modelValue);
  if (matched) {
    return matched.label;
  }
  return normalizedOptions.value[0]?.label ?? "";
});

function toggleOpen(): void {
  if (props.disabled) {
    return;
  }
  isOpen.value = !isOpen.value;
}

function choose(optionValue: string): void {
  emit("update:modelValue", optionValue);
  isOpen.value = false;
}

function closeMenu(): void {
  isOpen.value = false;
}

function onDocumentMouseDown(event: MouseEvent): void {
  if (!rootRef.value) {
    return;
  }
  if (!rootRef.value.contains(event.target as Node)) {
    closeMenu();
  }
}

function onDocumentKeyDown(event: KeyboardEvent): void {
  if (event.key === "Escape") {
    closeMenu();
  }
}

onMounted(() => {
  document.addEventListener("mousedown", onDocumentMouseDown);
  document.addEventListener("keydown", onDocumentKeyDown);
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocumentMouseDown);
  document.removeEventListener("keydown", onDocumentKeyDown);
});
</script>

<template>
  <div ref="rootRef" class="ui-select" :class="{ open: isOpen }">
    <button
      class="ui-select-trigger"
      type="button"
      :disabled="disabled"
      :aria-expanded="isOpen"
      :title="selectedLabel"
      @click="toggleOpen"
    >
      <span class="ui-select-value">{{ selectedLabel }}</span>
      <span class="ui-select-chevron" aria-hidden="true"></span>
    </button>

    <ul v-if="isOpen" class="ui-select-menu" role="listbox">
      <li v-for="option in normalizedOptions" :key="option.value">
        <button
          class="ui-select-option"
          :class="{ active: option.value === modelValue }"
          type="button"
          :title="option.label"
          @click="choose(option.value)"
        >
          {{ option.label }}
        </button>
      </li>
    </ul>
  </div>
</template>
