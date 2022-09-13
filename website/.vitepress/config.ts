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
  // head: [
  //   ['script', {async: '', src: 'https://www.googletagmanager.com/gtag/js?id=G-EZSJ3YF2RG'}],
  //   ['script', {}, gaScript],
  // ],
  outDir: './dist',
  themeConfig: {
    logo: 'logo.svg',
    nav: [
      { text: 'Guide', link: '/guide' },
      { text: 'Playground', link: '/playground' },
    ],
  },
})
