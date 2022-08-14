<template>
  <header>
    <a href="https://github.com/HerringtonDarkholme/ast-grep" target="_blank">
      <img src="/logo.svg" class="logo" alt="SG logo"/>
    </a>
    <div>
      <h1 class="description">SG playground: Write Code to Match Code</h1>
      <hr/>
      <ul>
        <li>
          <code>ast-grep</code> is a AST-based tool to search code by pattern code. Think it as your old-friend <code>grep</code> but it matches AST node instead of text.
        </li>
        <li>
          Write pattern as if write ordinary code. It will match all code that has the same syntactical structure.
        </li>
        <li>
          You can use <code>$</code> sign + upper case letters as wildcard, e.g. <code>$MATCH</code>, to match any single AST node. Think it as REGEX dot <code>.</code>, except it is not textual.
        </li>
        <li>
          You can use <code>$$$</code> to match multiple AST nodes.
        </li>
      </ul>
    </div>
  </header>
</template>
<style scoped>

header {
  display: flex;
}
.logo {
  height: 9em;
  padding: 1.5em;
  will-change: filter;
}
.logo:hover {
  filter: drop-shadow(0 0 2em #3e699166);
}
.description {
  text-align: left;
}

h1 {
  font-size: 2em;
}
code {
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, Courier, monospace;
  padding: 0.1em 0.2em;
  background-color: #ddd;
}
ul {
  list-style: disc inside;
}
hr {
  margin: 0.5em 0;
  border-top: 1px solid #eee;
}
</style>
