import { defineConfig } from 'vitepress'

const gaScript = `
window.dataLayer = window.dataLayer || [];
function gtag(){dataLayer.push(arguments);}
gtag('js', new Date());

gtag('config', 'G-EZSJ3YF2RG');
`

export default defineConfig({
  lang: 'en-US',
  title: 'ast-grep',
  titleTemplate: 'ligthning fast code tool',
  base: '/ast-grep/',
  description: 'ast-grep(sg) is a ligthning fast and user friendly tool for code searching, linting, rewriting at large scale.',
  head: [
    ['script', {async: '', src: 'https://www.googletagmanager.com/gtag/js?id=G-EZSJ3YF2RG'}],
    ['script', {}, gaScript],
  ],
  outDir: './dist',
  themeConfig: {
    logo: 'logo.svg',
    nav: [
      { text: 'Guide', link: '/guide/introduction' },
      { text: 'Reference', link: '/reference/cli' },
      { text: 'Playground', link: '/playground' },
    ],
    sidebar: [
      {
        text: 'Guide',
        items: [
          { text: 'Introduction', link: '/guide/introduction' },
          { text: 'Quick Start', link: '/guide/quick-start' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'Command Line Interface', link: '/reference/cli' },
          { text: 'YAML Configuration', link: '/reference/yaml' },
          { text: 'API Reference', link: '/reference/api' },
        ],
      },
      {
        text: 'Links',
        items: [
          { text: 'Playground', link: '/playground' },
          { text: 'Roadmap', link: '/links/roadmap' },
          { text: 'Docs.rs', link: 'https://docs.rs/ast-grep-core/0.1.2/ast_grep_core/' },
        ],
      },
    ],
  },
})
