// let a = 123


// async function foo(things) {
//   const results = [];
//   for (const thing of things) {
//     // Bad: each loop iteration is delayed until the entire asynchronous operation completes
//     results.push(await bar(thing));
//   }
//   return baz(results);
// }

// if (false) {
//   console.log('123')
// }

// class Test {
//   set test(test) {
//     return 'shoud hit'
//   }

//   test(test) {
//     return 'shoud not hit'
//   }
// }

Promise.all([
  await p1,  // match
  p2,        // no match
  another(await p1), // no match
  await p4,  // match
  ...[1,2,3].map(async (i) => await Promise.resolve(i)) // no match
])
