// Throughput of the original, on the same corpus the Rust benchmark uses.
//
// Only throughput is compared. Any single-shot latency number for a Node
// script is dominated by process startup and would say nothing about the
// library.

const path = require('path')
const semver = require(path.resolve(__dirname, '..', 'vendor', 'node-semver'))

const corpus = []
for (let i = 0; i < 1000; i++) {
  corpus.push(`${i % 50}.${i % 17}.${i % 7}-beta.${i % 3}`)
}

// Warm up so the JIT has compiled the hot path.
for (let i = 0; i < 20; i++) {
  for (const v of corpus) semver.parse(v)
}

const start = process.hrtime.bigint()
let parsed = 0
let elapsed = 0n
const budget = 500n * 1000n * 1000n // 500ms in nanoseconds

while (elapsed < budget) {
  for (const v of corpus) {
    semver.parse(v)
    parsed++
  }
  elapsed = process.hrtime.bigint() - start
}

const perSec = Number(BigInt(parsed) * 1000000000n / elapsed)
console.log(JSON.stringify({ parsed, perSec }))
