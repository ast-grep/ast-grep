import type { Rule } from '../manual'

export const createMatchClassMethodRule = (methodName: string): Rule => ({
  kind: 'method_signature',
  has: {
    field: 'name',
    regex: `^${methodName}$`,
  },
})

export const createMatchClassDeclarationRule = (className: string): Rule => ({
  kind: 'class_declaration',
  inside: {
    kind: 'ambient_declaration',
    inside: {
      kind: 'export_statement',
    },
  },
  has: {
    field: 'name',
    regex: `^${className}$`,
  },
})

export const createMatchSgReturningFunctionSignatureRule = (
  namespace: string,
): Rule => ({
  all: [
    {
      any: [
        {
          kind: 'type_annotation',
          regex: 'SgNode',
          inside: {
            kind: 'required_parameter',
            inside: {
              kind: 'function_type',
              stopBy: 'end',
            },
          },
        },
        {
          kind: 'type_annotation',
          regex: 'SgRoot',
          nthChild: {
            position: 1,
            reverse: true,
          },
          inside: {
            kind: 'function_signature',
          },
        },
      ],
    },
    {
      inside: {
        kind: 'internal_module',
        stopBy: 'end',
        has: {
          field: 'name',
          regex: `^${namespace}$`,
        },
      },
    },
  ],
})
