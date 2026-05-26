#!/usr/bin/env node
// validate-seeds.test.mjs — Falsifiability tests for validate-seeds.mjs
// Zero dependencies: uses node:test (built into Node 20).
//
// Each test generates a temp fixture from the real seed/ directory,
// injects one targeted mutation, and asserts the specific check fires.
// Cleanup is guaranteed by try/finally in the withFixture helper.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, cp, writeFile, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { validateSeeds } from './validate-seeds.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REAL_SEED_DIR = join(__dirname, '..', 'seed');
const REAL_SEED_ORDER = join(__dirname, 'seed-order.txt');

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/**
 * Create a temp directory with a copy of the real seed/ and optionally
 * mutated files. Returns the temp directory path.
 */
async function makeFixture(seedDir, mutations) {
  const tmp = await mkdtemp(join(tmpdir(), 'seed-test-'));
  await cp(seedDir, tmp, { recursive: true });
  for (const [file, content] of Object.entries(mutations)) {
    await writeFile(join(tmp, file), content);
  }
  return tmp;
}

/**
 * Run a test function inside a temp fixture with guaranteed cleanup.
 */
async function withFixture(seedDir, mutations, fn) {
  const tmp = await makeFixture(seedDir, mutations);
  try {
    await fn(tmp);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

// ---------------------------------------------------------------------------
// Helper: read a real seed file
// ---------------------------------------------------------------------------

async function readSeed(fileName) {
  return readFile(join(REAL_SEED_DIR, fileName), 'utf-8');
}

async function readSeedOrder() {
  const raw = await readFile(REAL_SEED_ORDER, 'utf-8');
  return raw.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('validate-seeds falsifiability', () => {

  // ── Control ─────────────────────────────────────────────────────────
  it('passes on unmodified real seeds (control)', async () => {
    await withFixture(REAL_SEED_DIR, {}, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      assert.strictEqual(result.failures.length, 0,
        'Unmodified seeds must produce zero failures. Got:\n' +
        result.failures.map(f => `  ${f.check}: ${f.message}`).join('\n'));
    });
  });

  // ── Check 1: column-count ───────────────────────────────────────────
  it('fails on column-count mismatch (extra value in jobs.sql)', async () => {
    const original = await readSeed('jobs.sql');
    // Add an extra value to the first row by appending before the closing paren
    const mutated = original.replace(
      /NULL,\s*NULL,\s*60,\s*FALSE,\s*NULL\s*\)/,
      "NULL, NULL, 60, FALSE, NULL, 'extra_val')"
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'jobs.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f => f.check === 'column-count');
      assert.ok(found,
        'Must detect column-count mismatch — this is the PR #59 extra-value bug');
    });
  });

  // ── Check 2: double-quote ──────────────────────────────────────────
  it('fails on double-quoted string in VALUES', async () => {
    const original = await readSeed('locations.sql');
    // Replace a single-quoted name with double-quoted
    const mutated = original.replace(
      "'Eddy''s Bedroom'",
      '"Eddy\'s Bedroom"'
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'locations.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f => f.check === 'double-quote');
      assert.ok(found,
        'Must detect double-quoted string in VALUES — PR #59 regression class');
    });
  });

  // ── Check 3: JSONB ─────────────────────────────────────────────────
  it('fails on invalid JSONB literal', async () => {
    const original = await readSeed('jobs.sql');
    // Replace valid jsonb with invalid json
    const mutated = original.replace(
      '\'{"typical_tasks": ["practice", "study", "perform"], "interfaces_with": ["professor", "writer"], "guardrails": ["Keep activities grounded in current locations and tools."], "contributor_notes": "Good for school, rehearsal, and late-night routine content."}\'::jsonb',
      "'{invalid json}'::jsonb"
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'jobs.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f => f.check === 'jsonb');
      assert.ok(found,
        'Must detect invalid JSONB literal — PR #59 regression class');
    });
  });

  // ── Check 4a: FK reference (missing agent) ─────────────────────────
  it('fails when FK reference target is missing (rosie_kim removed)', async () => {
    const original = await readSeed('agents.sql');
    // Remove the rosie_kim agent row — shops.sql still references her
    // Find the rosie_kim block and remove it
    const mutated = original.replace(
      /,\s*\(\s*'rosie_kim'[\s\S]*?'harvey_oak_checkout'\s*\)/,
      ''
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'agents.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' && f.message.includes('rosie_kim'));
      assert.ok(found,
        'Must detect dangling FK reference when rosie_kim is removed from agents.sql');
    });
  });

  // ── Check 4b: FK reference (order violation) ──────────────────────
  it('fails when seed-order.txt puts agents.sql after shops.sql', async () => {
    const orderLines = await readSeedOrder();
    // Move agents.sql to after shops.sql
    const withoutAgents = orderLines.filter(l => l !== 'agents.sql');
    const shopsIdx = withoutAgents.indexOf('shops.sql');
    withoutAgents.splice(shopsIdx + 1, 0, 'agents.sql');
    const mutatedOrder = withoutAgents.join('\n') + '\n';

    await withFixture(REAL_SEED_DIR, {}, async (tmp) => {
      const mutatedOrderPath = join(tmp, 'seed-order.txt');
      await writeFile(mutatedOrderPath, mutatedOrder);
      const result = await validateSeeds(tmp, { seedOrderPath: mutatedOrderPath });
      const found = result.failures.some(f => f.check === 'fk-ref');
      assert.ok(found,
        'Must detect FK ordering violation when agents.sql loads after shops.sql');
    });
  });

  // ── Check 5: adjacency symmetry ────────────────────────────────────
  it('fails when a reverse adjacency edge is removed', async () => {
    const original = await readSeed('adjacency.sql');
    // Remove one reverse edge: (lin_kitchen, lin_bedroom, 15)
    const mutated = original.replace(
      /\s*\('lin_kitchen',\s*'lin_bedroom',\s*15\),?/,
      ''
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'adjacency.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f => f.check === 'adjacency-symmetry');
      assert.ok(found,
        'Must detect missing reverse adjacency edge');
    });
  });

  // ── Check 6: inventory XOR ─────────────────────────────────────────
  it('fails when inventory row has both held_by and location_id set', async () => {
    const original = await readSeed('objects.sql');
    // Change a row to have both held_by and location_id set
    const mutated = original.replace(
      "('coffee_beans_001', 'Coffee Beans', NULL, 'hobbs_cafe_kitchen', '{}', 1, NULL, NULL, NULL)",
      "('coffee_beans_001', 'Coffee Beans', 'eddy_lin', 'hobbs_cafe_kitchen', '{}', 1, NULL, NULL, NULL)"
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'objects.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f => f.check === 'inventory-xor');
      assert.ok(found,
        'Must detect inventory XOR violation when both held_by and location_id are set');
    });
  });

  // ── Check 7: consumable integrity ──────────────────────────────────
  it('fails on invalid consumable_type', async () => {
    const original = await readSeed('objects.sql');
    // Change a consumable_type to an invalid value
    const mutated = original.replace(
      "('bread_001', 'Bread Loaf', NULL, 'harvey_oak_aisle', '{}', 5, 'food', 25, 150)",
      "('bread_001', 'Bread Loaf', NULL, 'harvey_oak_aisle', '{}', 5, 'magic', 25, 150)"
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'objects.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f =>
        f.check === 'consumable' && f.message.includes('magic'));
      assert.ok(found,
        'Must detect invalid consumable_type');
    });
  });

  // ── Check 7b: consumable integrity (bad vital_value) ────────────────
  it('fails on consumable with vital_value <= 0', async () => {
    const original = await readSeed('objects.sql');
    // Set vital_value to 0 on a consumable row
    const mutated = original.replace(
      "('bread_001', 'Bread Loaf', NULL, 'harvey_oak_aisle', '{}', 5, 'food', 25, 150)",
      "('bread_001', 'Bread Loaf', NULL, 'harvey_oak_aisle', '{}', 5, 'food', 0, 150)"
    );
    assert.notStrictEqual(original, mutated, 'Mutation must change the file');

    await withFixture(REAL_SEED_DIR, { 'objects.sql': mutated }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f =>
        f.check === 'consumable' && f.message.includes('vital_value'));
      assert.ok(found,
        'Must detect consumable with vital_value <= 0');
    });
  });

  // ── Check 8a: seed-order (unlisted file) ───────────────────────────
  it('fails when a .sql file exists in seed/ but is not in seed-order.txt', async () => {
    await withFixture(REAL_SEED_DIR, {
      'fake_table.sql': "INSERT INTO fake (id) VALUES ('test');"
    }, async (tmp) => {
      const result = await validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f =>
        f.check === 'seed-order' && f.message.includes('fake_table.sql'));
      assert.ok(found,
        'Must detect unregistered seed file not listed in seed-order.txt');
    });
  });

  // ── Check 8b: seed-order (missing file) ────────────────────────────
  it('fails when seed-order.txt lists a file that does not exist', async () => {
    const orderLines = await readSeedOrder();
    // Add a nonexistent file
    const mutatedOrder = [...orderLines, 'phantom.sql'].join('\n') + '\n';

    await withFixture(REAL_SEED_DIR, {}, async (tmp) => {
      const mutatedOrderPath = join(tmp, 'seed-order.txt');
      await writeFile(mutatedOrderPath, mutatedOrder);
      const result = await validateSeeds(tmp, { seedOrderPath: mutatedOrderPath });
      const found = result.failures.some(f =>
        f.check === 'seed-order' && f.message.includes('phantom.sql'));
      assert.ok(found,
        'Must detect seed file listed in seed-order.txt but missing from seed/');
    });
  });

});
