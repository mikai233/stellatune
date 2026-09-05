<script setup lang="ts">
defineOptions({
  name: "GatewayHeroCard"
});

defineProps<{
  pluginId: string;
  origin: string;
  hasGatewayContext: boolean;
  message: string;
}>();
</script>

<template>
  <section class="hero-card">
    <p class="hero-kicker">StellaTune 插件界面</p>
    <h1>网易云音源控制台</h1>
    <p class="hero-meta">
      插件 ID：<strong>{{ pluginId }}</strong>
    </p>
    <p class="hero-meta">
      插件服务：<strong>{{ origin }}</strong>
    </p>

    <p class="hero-message">{{ message }}</p>
  </section>
</template>
