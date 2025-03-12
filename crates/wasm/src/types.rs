use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(typescript_custom_section)]
const NODE_RANGE: &'static str = include_str!("../types.d.ts");

#[wasm_bindgen(typescript_custom_section)]
const MATCH: &'static str = r#"
type WasmNode<M> = {
  text: string;
  range: [number, number, number, number];
}

type SgMatch<M> = {
  id: number;
  node: WasmNode<M>;
  env: Map<string, WasmNode<M>>;
  message: string;
}
"#;
