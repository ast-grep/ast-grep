import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { Lang, parseAsync } from "../index";
import { NodeTypeSchema } from "../types/node-types";
import {
  createMatchClassDeclarationRule,
  createMatchClassMethodRule,
} from "./rules";
import {
  languageNodeTypesTagVersionOverrides,
  languagesCrateNames,
  languagesNodeTypesUrls,
} from "./constants";
import toml from "smol-toml";

const rootDir = path.resolve(__dirname, "..");
const indexDtsPath = path.join(rootDir, "index.d.ts");

async function generateLangNodeTypes() {
  const languageCargoToml = await readFile(
    path.resolve(rootDir, "../language/Cargo.toml"),
    "utf8"
  );

  const parsedCargoToml = toml.parse(languageCargoToml) as {
    dependencies: Record<string, { version: string }>;
  };

  for (const [lang, urlTemplate] of Object.entries(languagesNodeTypesUrls)) {
    try {
      const treeSitterCrateName = languagesCrateNames[lang as Lang];
      const cargoVersion =
        parsedCargoToml.dependencies[treeSitterCrateName].version;
      const tag =
        languageNodeTypesTagVersionOverrides[lang as Lang] ??
        `v${cargoVersion}`;
      const url = urlTemplate.replace("{{TAG}}", tag);
      const nodeTypesResponse = await fetch(url);
      const nodeTypes = (await nodeTypesResponse.json()) as NodeTypeSchema[];

      const nodeTypeMap = Object.fromEntries(
        nodeTypes.map((node) => [node.type, node])
      );

      await writeFile(
        path.join(rootDir, "types", `${lang}-node-types.ts`),
        `export type ${lang}NodeTypesMap = ${JSON.stringify(nodeTypeMap, null, 2)};`
      );
    } catch (e) {
      console.error(`Error while generating node types for ${lang}:`, e);
    }
  }
}

async function updateIndexDts() {
  const indexDtsSource = await readFile(indexDtsPath, "utf8");
  const sgRoot = await parseAsync(Lang.TypeScript, indexDtsSource);

  const root = sgRoot.root();

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

  const nodeTypesImportStatement =
    'import type { FieldNames, FieldSgNode, NodeTypesMap } from "./types/node-types";';
  const importStatements = [nodeTypesImportStatement];

  const exportStatements = Object.keys(languagesNodeTypesUrls)
    .map((lang) => `export type { ${lang}NodeTypesMap } from "./types/${lang}-node-types";`)

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
        insertedText: exportStatements.join("\n") + "\n",
      },
      sgRootClassTypeParametersRemovalEdit,
      sgNodeClassTypeParametersRemovalEdit,
      sgRootNameEdit,
      sgNodeClassNameEdit,
      isMethodEdit,
      fieldMethodEdit,
      fieldChildrenMethodEdit,
    ].filter((edit) => edit !== undefined)
  );

  await writeFile(indexDtsPath, updatedSource);
}

async function main() {
  await generateLangNodeTypes();
  await updateIndexDts();
}

main().catch((error) => {
  console.error('Error in main:', error);
  process.exit(1);
});