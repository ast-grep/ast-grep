import test from 'ava'

import {
  js, ts, tsx, html, Lang,
  parseFiles, parseAsync, findInFiles,
  parse as parseWithLang,
} from '../index'
const { parse, kind } = js
let parseMulti = countedPromise(parseFiles)


test('find from native code', t => {
  const sg = parse('console.log(123)')
  const match = sg.root().find('console.log')
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
  const node = match!.find('console')
  t.deepEqual(node!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 7, index: 7 },
  })
})

test('parse async', async t => {
  const sg = await js.parseAsync('console.log(123)')
  const match = sg.root().find('console.log')
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
  const node = match!.find('console')
  t.deepEqual(node!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 7, index: 7 },
  })
})

test('find multiple nodes', t => {
  const sg = parse('a(1, 2, 3)')
  const match = sg.root().find('a($$$B)')
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 10, index: 10 },
  })
  const matchedVar = match!.getMultipleMatches('B')
  let start = matchedVar[0].range().start;
  let end = matchedVar[matchedVar.length - 1].range().end;
  t.deepEqual(start, { line: 0, column: 2, index: 2 })
  t.deepEqual(end, { line: 0, column: 9, index: 9 })
})

test('find unicode', t => {
  const str = `console.log("Hello, 世界")
  print("ザ・ワールド")`
  const sg = parse(str)
  const match = sg.root().find('console.log($_)')
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 24, index: 24 },
  })
  const node = sg.root().find('print($_)')
  t.deepEqual(node!.range(), {
    start: { line: 1, column: 2, index: 27 },
    end: { line: 1, column: 17, index: 42 },
  })
})

test('find with transformation', t => {
  const str = `console.log("Hello, 世界")`
  const sg = parse(str)
  const match = sg.root().find({
    rule: {
      pattern: 'console.log($A)',
    },
    transform: {
      NEW_ARG: {
        substring: {
          source: '$A',
          startChar: 1,
          endChar: -1,
        }
      },
    }
  })!
  t.deepEqual(match.getTransformed('NEW_ARG'), 'Hello, 世界')
  t.deepEqual(match.getMatch('A')?.text(), '"Hello, 世界"')
})


test('test code fix', t => {
  const sg = parse('a = console.log(123)')
  const match = sg.root().find('console.log')!
  const fix = match.replace('console.error')
  t.deepEqual(fix.insertedText, 'console.error')
  t.deepEqual(fix.startPos, 4)
  t.deepEqual(fix.endPos, 15) // length of console.log
  t.deepEqual(match.commitEdits([fix]), 'console.error')
  const newCode = sg.root().commitEdits([fix])
  t.deepEqual(newCode, 'a = console.error(123)')
})

test('test code with multiple fixes', t => {
  // and also test unicode
  const sg = parse('いいよ = log(123) + log(456)')
  const matches = sg.root().findAll(js.kind('number'))
  const fixes = matches.map(match => match.replace('114514'))
  // make it in reverse order to test rust behavior
  fixes.sort((a, b) => b.startPos - a.startPos)
  const newCode = sg.root().commitEdits(fixes)
  t.deepEqual(newCode, 'いいよ = log(114514) + log(114514)')
})

test('test code fix with user defined range', t => {
  const sg = parse('いいよ = log(123)')
  const match = sg.root().find(js.kind('number'))!
  const edit = match.replace('514')
  edit.startPos -= 1
  edit.endPos += 1
  const newCode = sg.root().commitEdits([edit])
  t.deepEqual(newCode, 'いいよ = log514')
})

test('findAll from native code', t => {
  const sg = parse('console.log(123); let a = console.log.bind(console);')
  const match = sg.root().findAll('console.log')
  t.deepEqual(match.length, 2)
  t.deepEqual(match[0].range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
  t.deepEqual(match[1].range(), {
    start: { line: 0, column: 26, index: 26 },
    end: { line: 0, column: 37, index: 37 },
  })
})

test('find not match', t => {
  const sg = parse('console.log(123)')
  const match = sg.root().find('notExist')
  t.is(match, null)
})

test('get variable', t => {
  const sg = parse('console.log("hello world")')
  const match = sg.root().find('console.log($MATCH)')
  t.is(match!.getMatch('MATCH')!.text(), '"hello world"')
})

test('find by kind', t => {
  const sg = parse('console.log("hello world")')
  const match = sg.root().find(kind('member_expression'))
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
})

test('find by config', t => {
  const sg = parse('console.log("hello world")')
  const match = sg.root().find({
    rule: {kind: 'member_expression'},
  })
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
})

test('test find files', async t => {
  await parseMulti(['./__test__/index.spec.ts'], (err, tree) => {
    t.is(err, null)
    t.is(tree.filename(), './__test__/index.spec.ts')
    t.assert(tree.root() !== null)
  })
})

test('test file count', async t => {
  let i = 0
  let fileCount = await parseMulti(['./'], (err, _) => {
    // ZZZ... sleep a while to mock expensive operation
    let start = Date.now()
    while (Date.now() - start < 1) continue
    t.is(err, null)
    i++
  })
  t.is(i, fileCount)
})

test('test languageGlobs', async t => {
  let i = 0
  let foundVue = false
  let fileCount = await parseMulti({
    paths: ['./'],
    languageGlobs: {
      'html': ['*.vue']
    },
  }, (err, root) => {
    t.is(err, null)
    if (root.filename().endsWith('.vue')) {
      foundVue = true
    }
    i++
  })
  t.is(i, fileCount)
  t.true(foundVue)
})

test('show good error message for invalid arg', async t => {
  const sg = parse('console.log(123)')
  t.throws(() => sg.root().find({rule: {regex: '('}}), {
    message: /Rule contains invalid regex matcher/
  })
  t.throws(() => sg.root().find({
    rule: { all: [{any: [{ kind: '33'}]}]}
  }), {
    message: /Rule contains invalid kind matcher/
  })
})

test('find in files', async t => {
  let findInFiles = countedPromise(ts.findInFiles)
  await findInFiles({
    paths: ['./'],
    matcher: {
      rule: {kind: 'member_expression'}
    },
  }, (err, n) => {
    // ZZZ... sleep a while to mock expensive operation
    let start = Date.now()
    while (Date.now() - start < 1) continue
    t.is(err, null)
    t.assert(n.length > 0)
    t.assert(n[0].text().includes('.'))
  })
})

test('find in files with filename', async t => {
  let findInFiles = countedPromise(ts.findInFiles)
  await findInFiles({
    paths: ['./'],
    matcher: {
      rule: {kind: 'member_expression'},
    },
  }, (err, n) => {
    t.is(err, null)
    const root = n[0].getRoot();
    t.is(root.filename().replace('\\', '/'), './__test__/index.spec.ts')
  })
})

test('tsx should not find ts file', async t => {
  await tsx.findInFiles({
    paths: ['./'],
    matcher: {
      rule: {kind: 'member_expression'},
    },
  }, () => {
    t.assert(false)
  })
  t.assert(true)
})

test('find with language globs', async t => {
  let findInFiles = countedPromise(tsx.findInFiles)
  await findInFiles({
    paths: ['./'],
    matcher: {
      rule: {kind: 'member_expression'},
    },
    languageGlobs: ['*.ts'],
  }, (err, n) => {
    t.is(err, null)
    const root = n[0].getRoot();
    t.is(root.filename().replace('\\', '/'), './__test__/index.spec.ts')
  })
})

test('find with language globs can parse with correct language', async t => {
  let findInFiles = countedPromise(html.findInFiles)
  await findInFiles({
    paths: ['./'],
    matcher: {
      rule: {pattern: '<template>$A</template>'},
    },
    languageGlobs: ['*.vue'],
  }, (err, n) => {
    t.is(err, null)
    const root = n[0].getRoot();
    t.is(root.filename().replace('\\', '/'), './__test__/test.vue')
    const div = root.root().find('<h1>$A</h1>')?.getMatch('A')?.text()
    t.is(div, '{{ greeting }}')
  })
})


function countedPromise<F extends (t: any, cb: any) => Promise<number>>(func: F) {
  type P = Parameters<F>
  return async (t: P[0], cb: P[1]) => {
    let i = 0
    let fileCount: number | undefined = undefined
    let resolve = () => {} // will be called after all files are processed
    function wrapped(...args: any[]) {
      let ret = cb(...args)
      if (++i === fileCount) resolve()
      return ret
    }
    fileCount = await func(t, wrapped as P[1])
    // all files are not processed, wait the resolve function to be called
    if (fileCount > i) {
      await new Promise<void>(r => resolve = r)
    }
    return fileCount
  }
}

test('parse python', t => {
  const sg = parseWithLang(Lang.Python, 'print("hello world")')
  const node = sg.root().find('print')
  t.deepEqual(node!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 5, index: 5 },
  })
})

test('parse python async', async t => {
  const sg = await parseAsync(Lang.Python, 'print("hello world")')
  const node = sg.root().find('print')
  t.deepEqual(node!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 5, index: 5 },
  })
})

test('find rust', async t => {
  let changed = false
  const counted = countedPromise((t, cb) => findInFiles(Lang.Rust, t, cb))
  const num = await counted({
    paths: ['./'],
    matcher: {
      rule: {pattern: 'ast_grep_core'},
    },
  }, () => {
    changed = true
  })
  t.assert(changed)
  t.assert(num > 0)
})