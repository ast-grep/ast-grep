import { readFile, writeFile, stat } from 'node:fs/promises'
import path from 'node:path'
// gen type cannot be imported on CI due to un-generated napi binding
import type { Lang } from '../index'
import type { NodeType } from '../types/staticTypes'
import {
  languagesCrateNames,
  languagesNodeTypesUrls,
} from './constants'
import toml from 'smol-toml'

const rootDir = path.resolve(__dirname, '..')
const langDir = path.join(rootDir, 'lang')

async function fileExists(filePath: string): Promise<boolean> {
  try {
    await stat(filePath)
    return true
  } catch (_e) { // oxlint-disable-line eslint/no-unused-vars
    return false
  }
}

function filterOutUnNamedNode(node: NodeType): NodeType | null {
  if (!node.named) {
    return null
  }
  if (node.fields) {
    for (const field of Object.keys(node.fields)) {
      node.fields[field].types = node.fields[field].types.filter(n => n.named)
    }
  }
  if (node.children) {
    node.children.types = node.children.types.filter(n => n.named)
  }
  if (node.subtypes) {
    node.subtypes = node.subtypes.filter(n => n.named)
  }
  return node
}

function processNodeTypes(nodeTypes: NodeType[]): Record<string, NodeType> {
  const filteredNodeTypes = nodeTypes
    .map(filterOutUnNamedNode)
    .filter(node => !!node)
  const nodeTypeMap = Object.fromEntries(
    filteredNodeTypes.map(node => [node.type, node]),
  )
  return nodeTypeMap
}

async function generateLangNodeTypes() {
  const testOnly = process.argv.slice(2)[0]
  const languageCargoToml = await readFile(
    path.resolve(rootDir, '../language/Cargo.toml'),
    'utf8',
  )

  const parsedCargoToml = toml.parse(languageCargoToml) as {
    dependencies: Record<string, { version: string }>
  }

  let langs = Object.entries(languagesNodeTypesUrls) as [Lang, string][]
  // if we are running in test mode, we only want to generate types for TypeScript
  // and only if the file does not exist
  if (testOnly) {
    const existing = await fileExists(path.join(langDir, 'TypeScript.d.ts'))
    if (existing) {
      return
    }
    langs = langs.filter(([lang]) => lang === 'TypeScript')
  }

  for (const [lang, urlTemplate] of langs) {
    try {
      const treeSitterCrateName = languagesCrateNames[lang]
      const cargoVersion =
        parsedCargoToml.dependencies[treeSitterCrateName].version
      const tag = `v${cargoVersion}`
      const url = urlTemplate.replace('{{TAG}}', tag)
      const nodeTypesResponse = await fetch(url)
      const nodeTypes = (await nodeTypesResponse.json()) as NodeType[]
      const nodeTypeMap = processNodeTypes(nodeTypes)

      const fileContent =
        `// Auto-generated from tree-sitter ${lang} ${tag}` +
        '\n' +
        `type ${lang}Types = ${JSON.stringify(nodeTypeMap, null, 2)};` +
        '\n' +
        `export default ${lang}Types;`
      await writeFile(path.join(langDir, `${lang}.d.ts`), fileContent)
    } catch (e) {
      console.error(`Error while generating node types for ${lang}`)
      throw e
    }
  }
}

generateLangNodeTypes().catch(error => {
  console.error('Error:', error)
  process.exit(1)
})