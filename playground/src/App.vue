<script setup lang="ts">
import Playground from './components/Playground.vue'
import Intro from './components/Intro.vue'
</script>

<template>
  <div class="root">
    <Intro/>
    <Suspense>
      <Playground/>
      <template #fallback>
        <div class="loading">
          Loading Editor and Parser...
        </div>
      </template>
    </Suspense>
  </div>
</template>

<style scoped>
.logo {
  height: 9em;
  padding: 1.5em;
  will-change: filter;
}
.logo:hover {
  filter: drop-shadow(0 0 2em #3e699166);
}
.root {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  padding: 1rem 2rem;
  box-sizing: border-box;
}
.description {
  text-align: left;
}
.loading {
  display: flex;
  height: 100%;
  align-items: center;
  justify-content: center;
}
</style>
