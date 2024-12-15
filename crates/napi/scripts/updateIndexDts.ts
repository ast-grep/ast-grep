import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
// NOTE: we are not using the compiled napi binding in workspace
// because of it may not be available in the CI
// so we are using the napi package from npm
import { Lang, parseAsync } from '../index'
import {
  createMatchClassDeclarationRule,
  createMatchClassMethodRule,
} from './rules'

const rootDir = path.resolve(__dirname, '..')
const indexDtsPath = path.join(rootDir, 'index.d.ts')
async function updateIndexDts() {
  const indexDtsSource = await readFile(indexDtsPath, 'utf8')
  const sgRoot = await parseAsync(Lang.TypeScript, indexDtsSource)

  const root = sgRoot.root()

  const sgRootClass = root.find({
    rule: createMatchClassDeclarationRule('SgRoot'),
  })
  const sgRootClassTypeParametersRemovalEdit = sgRootClass!
    .field('type_parameters')
    ?.replace('')
  const sgRootNameEdit = sgRootClass!
    .field('name')!
    .replace('SgRoot<M extends NodeTypesMap = NodeTypesMap>')

  const sgNodeClass = root.find({
    rule: createMatchClassDeclarationRule('SgNode'),
  })

  const sgNodeClassTypeParametersRemovalEdit = sgNodeClass!
    .field('type_parameters')
    ?.replace('')
  const sgNodeClassNameEdit = sgNodeClass!.field('name')!.replace(`SgNode<
  M extends NodeTypesMap = NodeTypesMap,
  T extends string = keyof M
>`)

  const isMethodEdit = sgNodeClass!
    .find({
      rule: createMatchClassMethodRule('is'),
    })!
    .replace('is<K extends T>(kind: K): this is SgNode<M, K> & this')

  const fieldMethodEdit = sgNodeClass!
    .find({
      rule: createMatchClassMethodRule('field'),
    })!
    .replace('field<F extends FieldNames<M[T]>>(name: F): FieldSgNode<M, T, F>')

  const fieldChildrenMethodEdit = sgNodeClass!
    .find({
      rule: createMatchClassMethodRule('fieldChildren'),
    })!
    .replace(
      'fieldChildren<F extends FieldNames<M[T]>>(name: F): Exclude<FieldSgNode<M, T, F>, null>[]',
    )

  const nodeTypesImportStatement =
    'import type { FieldNames, FieldSgNode, NodeTypesMap } from "./types/node-types";'
  const importStatements = [nodeTypesImportStatement]

  const updatedSource = root.commitEdits(
    [
      {
        startPos: 0,
        endPos: 0,
        insertedText: importStatements.join('\n') + '\n',
      },
      sgRootClassTypeParametersRemovalEdit,
      sgNodeClassTypeParametersRemovalEdit,
      sgRootNameEdit,
      sgNodeClassNameEdit,
      isMethodEdit,
      fieldMethodEdit,
      fieldChildrenMethodEdit,
    ].filter(edit => edit !== undefined),
  )

  await writeFile(indexDtsPath, updatedSource)
}

updateIndexDts().catch(error => {
  console.error('Error in updateIndexDts:', error)
  process.exit(1)
})
