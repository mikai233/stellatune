<script setup lang="ts">
defineOptions({
  name: "GatewayHeroCard"
});

defineProps<{
  pluginId: string;
  origin: string;
  hasToken: boolean;
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
      网关来源：<strong>{{ origin }}</strong>
    </p>
    <p class="hero-meta">
      令牌：<strong>{{ hasToken ? "已提供" : "缺失" }}</strong>
    </p>
    <p class="hint warning" v-if="!hasGatewayContext">
      当前页面缺少网关会话参数。请从宿主打开：
      <code>/ui/dev.stellatune.source.netease</code>
      ，由网关自动注入 <code>token</code> 与 <code>gateway_origin</code>。
    </p>
    <p class="hero-message">{{ message }}</p>
  </section>
</template>
