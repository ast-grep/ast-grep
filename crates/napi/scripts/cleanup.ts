import { readdir, readFile, unlink, writeFile } from 'node:fs/promises'
import path from 'node:path'

const root = path.resolve(__dirname, '..')
const dirs = {
  root,
  types: path.join(root, 'types'),
  lang: path.join(root, 'lang'),
}

async function cleanup() {
  try {
    const files = await readdir(dirs.lang)

    await Promise.all(
      files
        .filter(file => file.endsWith('.d.ts'))
        .map(async file => {
          const filePath = path.join(dirs.lang, file)
          await unlink(filePath)
          console.log(`Deleted: ${filePath}`)
        }),
    )

    const existingTypesSource = await readFile(
      path.join(dirs.types, 'lang.d.ts'),
      'utf8',
    )

    const newSource = existingTypesSource.replace(
      /export type LanguageNodeTypes = \{[^{}]*\}/,
      'export type LanguageNodeTypes = Record<never, never>',
    )

    await writeFile(path.join(dirs.types, 'lang.d.ts'), newSource)
  } catch (e) {
    console.error('Error during cleanup:', e)
    throw e
  }
}

cleanup().catch(error => {
  console.error('Error:', error)
  process.exit(1)
})
