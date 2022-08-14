<script setup lang="ts">
import { ref, watch } from 'vue'
import Monaco from './Monaco.vue'
import TreeSitter from 'web-tree-sitter'
import Parser from 'web-tree-sitter'
import init, {find_nodes} from 'ast-grep-wasm'
import SelectLang from './SelectLang.vue'

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
let lang = ref('javascript')

const matchedHighlights = ref([])
watch([source, query], async ([source, query]) => {
  try {
    matchedHighlights.value = JSON.parse(await find_nodes(source, {pattern: query}))
  } catch (e) {
    matchedHighlights.value = []
  }
})

const count = ref(0)
</script>

<template>
  <SelectLang v-model="lang"/>
  <div class="playground">
    <div class="half">
      <Monaco @change="source = $event" :language="lang" :highlights="matchedHighlights"/>
    </div>
    <div class="half">
      <Monaco @change="query = $event"/>
    </div>
  </div>
  <div v-if="matchedHighlights.length > 0">Found {{ matchedHighlights.length }} match(es).</div>
  <div v-else>No match found.</div>
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
