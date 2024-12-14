import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { Edit, kind, Lang, parseAsync } from "../index";
import { Rule } from "../manual";
import { NodeTypeSchema } from "../types/node-types";

const languagesNodeTypesUrls = {
  [Lang.JavaScript]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-javascript/refs/heads/master/src/node-types.json",
  [Lang.TypeScript]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/refs/heads/master/typescript/src/node-types.json",
  [Lang.Tsx]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/refs/heads/master/tsx/src/node-types.json",
  [Lang.Java]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-java/refs/heads/master/src/node-types.json",
  [Lang.Python]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/refs/heads/master/src/node-types.json",
  [Lang.Rust]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-rust/refs/heads/master/src/node-types.json",
  [Lang.C]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-c/refs/heads/master/src/node-types.json",
  [Lang.Cpp]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-cpp/refs/heads/master/src/node-types.json",
  [Lang.Go]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-go/refs/heads/master/src/node-types.json",
  [Lang.Html]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-html/refs/heads/master/src/node-types.json",
  [Lang.Css]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-css/refs/heads/master/src/node-types.json",
  [Lang.Json]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-json/refs/heads/master/src/node-types.json",
  [Lang.CSharp]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-c-sharp/refs/heads/master/src/node-types.json",
  [Lang.Ruby]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-ruby/refs/heads/master/src/node-types.json",
  [Lang.Php]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-php/refs/heads/master/php/src/node-types.json",
  [Lang.Elixir]:
    "https://raw.githubusercontent.com/elixir-lang/tree-sitter-elixir/refs/heads/main/src/node-types.json",
  [Lang.Kotlin]:
    "https://raw.githubusercontent.com/fwcd/tree-sitter-kotlin/refs/heads/main/src/node-types.json",
  [Lang.Swift]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-swift/refs/heads/master/src/node-types.json",
  [Lang.Haskell]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-haskell/refs/heads/master/src/node-types.json",
  [Lang.Scala]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-scala/refs/heads/master/src/node-types.json",
  [Lang.Lua]:
    "https://raw.githubusercontent.com/tjdevries/tree-sitter-lua/refs/heads/master/src/node-types.json",
  [Lang.Bash]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-bash/refs/heads/master/src/node-types.json",
  [Lang.Yaml]:
    "https://raw.githubusercontent.com/ikatyang/tree-sitter-yaml/refs/heads/master/src/node-types.json",
  // Not available for SQL
};

const dirname = new URL(".", import.meta.url).pathname;

async function generateLangNodeTypes() {
  for (const [lang, url] of Object.entries(languagesNodeTypesUrls)) {
    const nodeTypesResponse = await fetch(url);
    const nodeTypes = (await nodeTypesResponse.json()) as NodeTypeSchema[];

    const nodeTypeMap = Object.fromEntries(
      nodeTypes.map((node) => [node.type, node])
    );

    await writeFile(
      path.join(dirname, "..", "types", `${lang}-node-types.ts`),
      `type ${lang}NodeTypesMap = ${JSON.stringify(nodeTypeMap, null, 2)};

export default ${lang}NodeTypesMap;
`
    );
  }
}

async function updateIndexDts() {
  const indexDtsPath = path.join(dirname, "..", "index.d.ts");
  const indexDtsSource = await readFile(indexDtsPath, "utf8");
  const sgRoot = await parseAsync(Lang.TypeScript, indexDtsSource);

  const root = sgRoot.root();

  const createMatchClassMethodRule = (methodName: string): Rule => ({
    kind: "method_signature",
    has: {
      field: "name",
      regex: `^${methodName}$`,
    },
  });

  const createMatchClassDeclarationRule = (className: string): Rule => ({
    kind: "class_declaration",
    inside: {
      kind: "ambient_declaration",
      inside: {
        kind: "export_statement",
      },
    },
    has: {
      field: "name",
      regex: `^${className}$`,
    },
  });

  const createMatchSgReturningFunctionSignatureRule = (
    namespace: string
  ): Rule => ({
    all: [
      {
        any: [
          {
            kind: "type_annotation",
            regex: "SgNode",
            inside: {
              kind: "required_parameter",
              inside: {
                kind: "function_type",
                stopBy: "end",
              }
            },
          },
          {
            kind: "type_annotation",
            regex: "SgRoot",
            nthChild: {
              position: 1,
              reverse: true,
            },
            inside: {
              kind: "function_signature",
            },
          },
        ],
      },
      {
        inside: {
          kind: "internal_module",
          stopBy: "end",
          has: {
            field: "name",
            regex: `^${namespace}$`,
          },
        },
      },
    ],
  });

  const sgRootClass = root.find({
    rule: createMatchClassDeclarationRule("SgRoot"),
  });
  const sgRootClassTypeParametersRemovalEdit = sgRootClass!
    .field("type_parameters")
    ?.replace("");
  const sgRootNameEdit = sgRootClass!
    .field("name")!
    .replace("SgRoot<M extends NodeTypesMap = NodeTypesMap>");

  const sgNodeClass = root.find({
    rule: createMatchClassDeclarationRule("SgNode"),
  });

  const sgNodeClassTypeParametersRemovalEdit = sgNodeClass!
    .field("type_parameters")
    ?.replace("");
  const sgNodeClassNameEdit = sgNodeClass!.field("name")!.replace(`SgNode<
  M extends NodeTypesMap = NodeTypesMap,
  T extends keyof M = keyof M
>`);

  const isMethodEdit = sgNodeClass!
    .find({
      rule: createMatchClassMethodRule("is"),
    })!
    .replace(`is<K extends T>(kind: K): this is SgNode<M, K> & this`);

  const fieldMethodEdit = sgNodeClass!
    .find({
      rule: createMatchClassMethodRule("field"),
    })!
    .replace(
      `field<F extends FieldNames<M[T]>>(name: F): FieldSgNode<M, T, F>`
    );

  const fieldChildrenMethodEdit = sgNodeClass!
    .find({
      rule: createMatchClassMethodRule("fieldChildren"),
    })!
    .replace(
      `fieldChildren<F extends FieldNames<M[T]>>(name: F): Exclude<FieldSgNode<M, T, F>, null>[]`
    );

  const langLibs = {
    js: Lang.JavaScript,
    jsx: Lang.JavaScript,
    ts: Lang.TypeScript,
    tsx: Lang.Tsx,
    css: Lang.Css,
    html: Lang.Html,
  };

  const updateLibEdits: Edit[] = [];

  for (const [ns, lang] of Object.entries(langLibs)) {
    const langNodeTypesMapNodes = root.findAll({
      rule: createMatchSgReturningFunctionSignatureRule(ns),
    })!;

    for (const langNodeTypesMap of langNodeTypesMapNodes) {
      const edit = langNodeTypesMap.replace(
        langNodeTypesMap
          ?.text()
          .replace(/SgRoot|SgNode/g, (match) => `${match}<${lang}NodeTypesMap>`)
      );
      updateLibEdits.push(edit);
    }
  }

  const typesImportStatement =
    'import type { FieldNames, FieldSgNode, NodeTypesMap } from "./types/node-types";';

  const importStatements = [typesImportStatement];
  for (const lang of Object.keys(languagesNodeTypesUrls)) {
    importStatements.push(
      `import ${lang}NodeTypesMap from "./types/${lang}-node-types";`
    );
  }
  const exportStatement = `export {\n${Object.keys(languagesNodeTypesUrls)
    .map((lang) => `  ${lang}NodeTypesMap`)
    .join(",\n")}\n};`;

  const updatedSource = root.commitEdits(
    [
      {
        startPos: 0,
        endPos: 0,
        insertedText: importStatements.join("\n") + "\n",
      },
      {
        startPos: 0,
        endPos: 0,
        insertedText: exportStatement + "\n",
      },
      sgRootClassTypeParametersRemovalEdit,
      sgNodeClassTypeParametersRemovalEdit,
      sgRootNameEdit,
      sgNodeClassNameEdit,
      isMethodEdit,
      fieldMethodEdit,
      fieldChildrenMethodEdit,
      ...updateLibEdits,
    ].filter((edit) => edit !== undefined)
  );

  await writeFile(indexDtsPath, updatedSource);
}

async function main() {
  await generateLangNodeTypes();
  await updateIndexDts();
}

void main();
