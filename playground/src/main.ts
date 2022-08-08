import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
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
console.log(
  await find_nodes('let a = 123')
)

createApp(App).mount('#app')
