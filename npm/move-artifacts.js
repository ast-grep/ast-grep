const fs = require('fs');

const dir = `${__dirname}/..`;

// Add `libc` fields only to platforms that have libc(Standard C library).
const triples = [
  {
    name: 'x86_64-apple-darwin',
  },
  {
    name: 'x86_64-unknown-linux-gnu',
  },
  {
    name: 'x86_64-pc-windows-msvc',
  },
  {
    name: 'aarch64-apple-darwin',
  },
];
const cpuToNodeArch = {
  x86_64: 'x64',
  aarch64: 'arm64',
};
const sysToNodePlatform = {
  linux: 'linux',
  darwin: 'darwin',
  windows: 'win32',
};

for (let triple of triples) {
  let [cpu, , os, abi] = triple.name.split('-');
  cpu = cpuToNodeArch[cpu] || cpu;
  os = sysToNodePlatform[os] || os;

  let t = `${os}-${cpu}`;
  if (abi) {
    t += '-' + abi;
  }

  buildCLI(triple.name, os, t);
}


function buildCLI(triple, os, t) {
  console.log('-------', triple, os, t, '-------')
  let binary = os === 'win32' ? 'sg.exe' : 'sg';
  fs.copyFileSync(`${dir}/artifacts/bindings-${triple}/${binary}`, `${dir}/npm/platforms/${t}/${binary}`);
  fs.chmodSync(`${dir}/npm/platforms/${t}/${binary}`, 0o755); // Ensure execute bit is set.
}
