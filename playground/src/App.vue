<script setup lang="ts">
import Playground from './components/Playground.vue'
import Intro from './components/Intro.vue'
</script>

<template>
  <div class="root">
    <Intro/>
    <Suspense>
      <Playground/>
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
</style>
