<script setup lang="ts">
import Playground from './components/Playground.vue'
</script>

<template>
  <div class="root">
    <a href="https://vitejs.dev" target="_blank">
      <img src="/logo.svg" class="logo" alt="SG logo" />
    </a>
    <h1>SG: ast-grep</h1>
    <Suspense>
      <Playground/>
    </Suspense>
  </div>
</template>

<style scoped>
.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
}
.logo:hover {
  filter: drop-shadow(0 0 2em #646cffaa);
}
.logo.vue:hover {
  filter: drop-shadow(0 0 2em #42b883aa);
}
.root {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  padding: 2rem;
  box-sizing: border-box;
}
</style>
