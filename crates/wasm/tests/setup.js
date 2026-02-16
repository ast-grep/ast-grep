const path = require("path");

exports.parserPath = function (lang) {
  return require.resolve(`tree-sitter-${lang}/tree-sitter-${lang}.wasm`);
};
