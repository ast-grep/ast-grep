//-----Type Only Export!-----//
export type { Pos, Edit, Range } from './types/sgnode'
export type { NapiConfig, FindConfig, FileOption } from './types/config'
// Only Rule here. User can use Rule['pattern'], e.g., to get the type of subfield.
export type { Rule } from './types/rule'

//-----Runtime Value Export!-----//
export { SgRoot, SgNode } from './types/sgnode'
export { Lang } from './types/lang'
export {
  parseFiles,
  parse,
  parseAsync,
  kind,
  pattern,
  findInFiles,
} from './types/api'
// deprecated
export * from './types/deprecated'
