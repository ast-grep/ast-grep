<script setup lang="ts">
import { ref, watchEffect } from 'vue'
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

let source = ref(
`function tryAstGrep() {
    console.log('hello world')
}`)
let query = ref('console.log($MATCH)')
let lang = ref('javascript')

const matchedHighlights = ref([])
watchEffect(async () => {
  try {
    matchedHighlights.value = JSON.parse(await find_nodes(source.value, {pattern: query.value}))
  } catch (e) {
    matchedHighlights.value = []
  }
  return () => {}
})

const count = ref(0)
</script>

<template>
  <div class="editor-captions">
    <div class="half editor-caption">
      Test Code
    </div>
    <div class="">
      Pattern
    </div>
    <SelectLang class="selector" v-model="lang"/>
  </div>
  <div class="playground">
    <div class="half">
      <Monaco v-model="source" :highlights="matchedHighlights"/>
    </div>
    <div class="half">
      <Monaco v-model="query"/>
    </div>
  </div>
  <p v-if="matchedHighlights.length > 0">Found {{ matchedHighlights.length }} match(es).</p>
  <p v-else>No match found.</p>
</template>

<style scoped>
.playground {
  display: flex;
  flex-wrap: wrap;
  flex: 1 0 auto;
}
.half {
  width: 50%;
}
.editor-captions {
  display: flex;
  text-align: left;
}
.selector {
  margin-left: auto;
}
.editor-caption {
  flex: 0 0 auto;
  height: 1.5em;
}
p {
  margin-top: 1em;
}
</style>
