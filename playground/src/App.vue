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
          <svg class="lightning-icon" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100" height="120" viewBox="0 10 100 120">
            <g transform="matrix(1,0,0,1,0,22)" >
            <polygon xmlns="http://www.w3.org/2000/svg" points="100,0 25,50 43.75,62.5 0,100 75,62.5 56.25,50"></polygon>
            </g>
          </svg>
          <h2>
            Loading Editor and Parser...
          </h2>
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
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

h2 {
  font-size: 20px;
  margin-top: 5vh;
}

@keyframes lightning-flash {
  0% {
    stroke-dashoffset: 1000;
    fill: #ffe91000;
  }
  20% {
    stroke-dashoffset: 800;
    fill: #ffe91000;
  }
  50% {
    stroke-dashoffset: 0;
    fill: #ffe910ff;
    opacity: 1;
  }
  60% {
    filter: drop-shadow(0 0 0px #fcf1ab00);
    opacity: 0;
  }
  70% {
    opacity: 1;
  }
  80% {
    opacity: 0;
  }
  90% {
    opacity: 1;
  }
  100% {
    filter: drop-shadow(0 0 20px #fcf1abff);
  }
}
.lightning-icon {
  fill: #ffe910ff;
  stroke-dasharray: 1000;
  stroke-width: 2px;
  stroke: var(--theme-highlight3);
  transform: rotate(-35deg);
  animation: linear 4s lightning-flash forwards infinite;
}
</style>
