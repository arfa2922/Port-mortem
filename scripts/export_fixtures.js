#!/usr/bin/env node
'use strict'

// Convert the original's JavaScript fixtures into language-neutral JSON.
//
// The fixtures are the equivalence oracle: plain arrays of inputs and
// expected outputs. Exporting them to JSON loses nothing and lets the
// Rust port assert against exactly the cases the original asserts
// against, rather than a re-typed approximation of them.
//
// Some fixtures import constants from the source tree, so this runs
// against a full checkout under vendor/. The pinned copy in
// tests/original/ is never written to — its hash must stay valid.
//
// Usage:  node scripts/export_fixtures.js

const fs = require('fs')
const path = require('path')

const ROOT = path.resolve(__dirname, '..')
const VENDOR = path.join(ROOT, 'vendor', 'node-semver')
const FIXTURE_DIR = path.join(VENDOR, 'test', 'fixtures')
const OUT = path.join(ROOT, 'tests', 'fixtures.json')

if (!fs.existsSync(FIXTURE_DIR)) {
  console.error(`error: ${FIXTURE_DIR} not found`)
  console.error('run: bash scripts/fetch_original.sh')
  process.exit(1)
}

const out = {}
let total = 0

for (const file of fs.readdirSync(FIXTURE_DIR).sort()) {
  if (!file.endsWith('.js')) continue
  const name = path.basename(file, '.js')
  let data
  try {
    data = require(path.join(FIXTURE_DIR, file))
  } catch (e) {
    console.error(`  skip ${name}: ${e.message.split('\n')[0]}`)
    continue
  }
  out[name] = data
  const n = Array.isArray(data) ? data.length : Object.keys(data).length
  total += n
  console.log(`  ${String(n).padStart(4)}  ${name}`)
}

fs.writeFileSync(OUT, JSON.stringify(out, null, 2) + '\n')
console.log('')
console.log(`${total} cases across ${Object.keys(out).length} fixtures`)
console.log(`wrote ${path.relative(ROOT, OUT)}`)
