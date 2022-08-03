let a = 123


async function foo(things) {
  const results = [];
  for (const thing of things) {
    // Bad: each loop iteration is delayed until the entire asynchronous operation completes
    results.push(await bar(thing));
  }
  return baz(results);
}

if (false) {
  console.log('123')
}

class Test {
  set test(test) {
    return 'shoud hit'
  }

  test(test) {
    return 'shoud not hit'
  }
}
