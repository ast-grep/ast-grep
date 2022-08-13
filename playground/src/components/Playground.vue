<script setup lang="ts">
import { ref, watch } from 'vue'
import Monaco from './Monaco.vue'
import TreeSitter from 'web-tree-sitter'
import Parser from 'web-tree-sitter'
import init, {find_nodes} from 'ast-grep-wasm'

async function initializeTreeSitter() {
  await TreeSitter.init()
  let entrypoint = globalThis as any
  entrypoint.Parser = TreeSitter
  entrypoint.Language = TreeSitter.Language
}

await initializeTreeSitter()
await init()

let source = ref(``)
let query = ref('')
const result = ref('')
watch([source, query], async ([source, query]) => {
  console.log(source, query)
  result.value = await find_nodes(source, {pattern: query})
})

const count = ref(0)
</script>

<template>
  <div class="playground">
    <div class="half">
      <Monaco @change="source = $event"/>
    </div>
    <div class="half">
      <Monaco @change="query = $event"/>
    </div>
  </div>
  <div>{{ result }}</div>
</template>

<style scoped>
.playground {
  display: flex;
  flex: 1 0 auto;
}
.half {
  width: 50%;
}
</style>
