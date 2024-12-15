import { readFile, writeFile, stat } from "node:fs/promises";
import path from "node:path";
import { Lang } from "../index";
import { NodeTypeSchema } from "../types/node-types";
import {
  languageNodeTypesTagVersionOverrides,
  languagesCrateNames,
  languagesNodeTypesUrls,
} from "./constants";
import toml from "smol-toml";

const rootDir = path.resolve(__dirname, "..");
const langDir = path.join(rootDir, "lang")

async function fileExists(filePath: string): Promise<boolean> {
  try {
    await stat(filePath);
    return true;
  } catch (e) {
    return false;
  }
}

async function generateLangNodeTypes() {
  const testOnly = process.argv.slice(2)[0];
  const languageCargoToml = await readFile(
    path.resolve(rootDir, "../language/Cargo.toml"),
    "utf8"
  );

  const parsedCargoToml = toml.parse(languageCargoToml) as {
    dependencies: Record<string, { version: string }>;
  };

  let langs = Object.entries(languagesNodeTypesUrls);
  // if we are running in test mode, we only want to generate types for TypeScript
  // and only if the file does not exist
  if (testOnly) {
    let existing = await fileExists(path.join(langDir, `${Lang.TypeScript}.d.ts`));
    if (existing) {
      return
    }
    langs = langs.filter(([lang]) => lang === Lang.TypeScript);
  }

  for (const [lang, urlTemplate] of langs) {
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

      const fileContent =
        `// Auto-generated from tree-sitter ${lang} ${tag}` + '\n' +
        `type ${lang}Types = ${JSON.stringify(nodeTypeMap, null, 2)};` + '\n' +
        `export default ${lang}Types;`;
      await writeFile(
        path.join(langDir, `${lang}.d.ts`),
        fileContent,
      );
    } catch (e) {
      console.error(`Error while generating node types for ${lang}`);
      throw e
    }
  }
}

generateLangNodeTypes().catch((error) => {
  console.error('Error:', error);
  process.exit(1);
});