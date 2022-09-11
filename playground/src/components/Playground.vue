<script setup lang="ts">
import { ref, watchEffect } from 'vue'
import Monaco from './Monaco.vue'
import TreeSitter from 'web-tree-sitter'
import init, {find_nodes, setup_parser} from 'ast-grep-wasm'

async function initializeTreeSitter() {
  await TreeSitter.init()
  let entrypoint = globalThis as any
  entrypoint.Parser = TreeSitter
  entrypoint.Language = TreeSitter.Language
}

await initializeTreeSitter()
await init()

let source = ref(
`/* All console.log() call will be highlighted!*/

function tryAstGrep() {
  console.log('Hello World')
}

const multiLineExpression =
  console
   .log('Also matched!')

const notThis = 'console.log("not me")'`
)
let query = ref('console.log($MATCH)')
let lang = ref('javascript')
let langLoaded = ref(false)

const matchedHighlights = ref([])
const parserPaths: Record<string, string> = {
  javascript: 'tree-sitter-javascript.wasm',
  typescript: 'tree-sitter-typescript.wasm',
}

function doFind() {
  return find_nodes(
    source.value,
    {pattern: query.value},
  )
}

watchEffect(async () => {
  langLoaded.value = false
  await setup_parser(parserPaths[lang.value])
  langLoaded.value = true
})

watchEffect(async () => {
  try {
    if (!langLoaded.value) {
      return () => {}
    }
    matchedHighlights.value = JSON.parse(await doFind())
  } catch (e) {
    matchedHighlights.value = []
  }
  return () => {}
})

function todo() {
  window.alert('Under construction...')
}

</script>

<template>
  <main class="playground">
    <div class="half">
      <div class="editor-caption">
        <a class="tab active">
          Test Code
        </a>
        <p class="match-result">
          <span  v-if="matchedHighlights.length > 0">
            Found {{ matchedHighlights.length }} match(es).
          </span>
          <span v-else>No match found.</span>
        </p>
      </div>
      <div class="editor-wrapper">
        <Monaco v-model="source" :language="lang" :highlights="matchedHighlights"/>
      </div>
    </div>
    <div class="half">
      <div class="editor-caption">
        <a class="tab active">
          Pattern Code
        </a>
        <a class="tab" @click="todo">
          Rule Config
        </a>
      </div>
      <div class="editor-wrapper">
        <Monaco v-model="query"/>
      </div>
    </div>
  </main>
</template>

<style scoped>
.playground {
  display: flex;
  flex-wrap: wrap;
  flex: 1 0 auto;
  align-items: stretch;
}
.half {
  flex: 1 0 30%;
  display: flex;
  flex-direction: column;
  filter: drop-shadow(0 0 1px #00000010);
}
.half:first-child {
  margin-right: 10px;
}
.half:focus-within {
  filter: drop-shadow(0 0 10px #00000015);
}
.selector {
  margin-left: auto;
}
.editor-caption {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  margin-bottom: -1px;
}
.tab {
  border-radius: 5px 5px 0 0;
  border: 1px solid #eee;
  padding: 8px 16px;
  cursor: pointer;
  z-index: 2;
  color: inherit;
  opacity: 0.7;
}
.tab.active {
  background-color: #fff;
  position: relative;
  border-bottom-color: #fff;
  margin-right: 0.2em;
  opacity: 1;
  color: var(--brand-color);
}
.editor-wrapper {
  flex: 1 0 auto;
  border: 1px solid #eee;
}
.match-result {
  margin-left: auto;
  margin-right: 1em;
}
</style>
