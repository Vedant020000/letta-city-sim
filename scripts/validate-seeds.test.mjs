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

import { validateSeeds, deriveFkRules } from './validate-seeds.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REAL_SEED_DIR = join(__dirname, '..', 'seed');
const REAL_SEED_ORDER = join(__dirname, 'seed-order.txt');
const REAL_MIGRATIONS_DIR = join(__dirname, '..', 'world-api', 'migrations');

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

/**
 * Create a temp seed directory from scratch (NOT a copy of the real seeds),
 * write the given { fileName: content } map into it, run fn(tmpDir), then
 * clean up. FK rules are derived from the REAL migrations by default (the
 * validator resolves migrationsDir relative to the script, not the seed dir),
 * which is what we want: small hand-built seed sets validated against the
 * actual schema.
 */
async function withTempSeeds(files, fn) {
  const tmp = await mkdtemp(join(tmpdir(), 'seed-temp-'));
  try {
    for (const [name, content] of Object.entries(files)) {
      await writeFile(join(tmp, name), content);
    }
    await fn(tmp);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

/**
 * Create a temp migrations directory with the given { fileName: content }
 * map, run fn(tmpDir), then clean up. Used to exercise the migration FK
 * parser in isolation.
 */
async function withTempMigrations(files, fn) {
  const tmp = await mkdtemp(join(tmpdir(), 'migs-temp-'));
  try {
    for (const [name, content] of Object.entries(files)) {
      await writeFile(join(tmp, name), content);
    }
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
      const found = result.failures.some(f => f.check === 'column-count');
      assert.ok(found,
        'Must detect column-count mismatch — this is the PR #59 extra-value bug');
    });
  });

  // ── Check 1b: line numbers ─────────────────────────────────────────
  it('reports the line for the matching INSERT, not an earlier same-prefix INSERT', async () => {
    const tmp = await mkdtemp(join(tmpdir(), 'seed-test-'));
    const sql = [
      "INSERT INTO jobs (id, title) VALUES ('first_job', 'First Job');",
      "",
      "-- This second INSERT has the same prefix but is the failing one.",
      "INSERT INTO jobs (id, title) VALUES ('second_job', 'Second Job', 'extra');",
      "",
    ].join('\n');

    try {
      await writeFile(join(tmp, 'jobs.sql'), sql);
      await writeFile(join(tmp, 'seed-order.txt'), 'jobs.sql\n');

      const result = validateSeeds(tmp, {
        seedOrderPath: join(tmp, 'seed-order.txt'),
      });

      const failure = result.failures.find(f => f.check === 'column-count');
      assert.ok(failure, 'Must detect the second INSERT column-count mismatch');
      assert.strictEqual(failure.line, 4,
        'Column-count failure should point to the second INSERT line');
    } finally {
      await rm(tmp, { recursive: true, force: true });
    }
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: mutatedOrderPath });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: REAL_SEED_ORDER });
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
      const result = validateSeeds(tmp, { seedOrderPath: mutatedOrderPath });
      const found = result.failures.some(f =>
        f.check === 'seed-order' && f.message.includes('phantom.sql'));
      assert.ok(found,
        'Must detect seed file listed in seed-order.txt but missing from seed/');
    });
  });

  // ── FK rule coverage ───────────────────────────────────────────────
  //
  // The previous manual FK list covered these seeded relationships.
  // Migration-derived rules should continue to cover the same source/target
  // pairs. Nullability is tested separately because the old nullable flags
  // were not enforced by the FK check.
  it('derived FK rules cover the previously listed relationships', () => {
    const expected = [
      ['location_adjacency', 'from_id', 'locations', 'id'],
      ['location_adjacency', 'to_id', 'locations', 'id'],
      ['world_objects', 'location_id', 'locations', 'id'],
      ['inventory_items', 'location_id', 'locations', 'id'],
      ['inventory_items', 'held_by', 'agents', 'id'],
      ['agents', 'current_location_id', 'locations', 'id'],
      ['agents', 'home_location_id', 'locations', 'id'],
      ['jobs', 'employer_id', 'agents', 'id'],
      ['agent_jobs', 'agent_id', 'agents', 'id'],
      ['agent_jobs', 'job_id', 'jobs', 'id'],
      ['location_roles', 'location_id', 'locations', 'id'],
      ['location_roles', 'agent_id', 'agents', 'id'],
      ['shops', 'owner_id', 'agents', 'id'],
      ['shops', 'shopkeeper_job_id', 'jobs', 'id'],
      ['banks', 'banker_job_id', 'jobs', 'id'],
      ['banks', 'updated_by', 'agents', 'id'],
    ];

    const derived = deriveFkRules(REAL_MIGRATIONS_DIR);
    const derivedKeys = new Set(
      derived.map(r => `${r.sourceTable}.${r.column}->${r.targetTable}.${r.targetColumn}`)
    );

    const missing = expected
      .map(([st, c, tt, tc]) => `${st}.${c}->${tt}.${tc}`)
      .filter(k => !derivedKeys.has(k));

    assert.deepStrictEqual(missing, [],
      'Derived FK rules no longer cover: ' + missing.join(', '));
  });

  // ── construction_projects coverage ─────────────────────────────────
  //
  // construction_projects.agent_id references agents(id) in
  // 0020_construction.sql. construction_projects was not present in the
  // previous manual rule list, so a bad agent_id there went unchecked.
  // The derived rules should catch it.
  it('fails on construction_projects with a bad agent_id', async () => {
    const seedFiles = {
      'agents.sql':
        "INSERT INTO agents (id, current_location_id) VALUES ('eddy_lin', 'lin_bedroom');\n",
      'locations.sql':
        "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      'construction_companies.sql':
        "INSERT INTO construction_companies (id) VALUES ('smallville_construction');\n",
      'construction_projects.sql':
        "INSERT INTO construction_projects (id, agent_id, company_id, location_id) " +
        "VALUES ('proj_1', 'NOBODY', 'smallville_construction', 'lin_bedroom');\n",
      'seed-order.txt':
        'locations.sql\nagents.sql\nconstruction_companies.sql\nconstruction_projects.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' && f.message.includes('NOBODY'));
      assert.ok(found,
        'Derived rules should catch a bad construction_projects.agent_id');
    });
  });

  // Positive control: a fully valid construction_projects row should not
  // false-positive on any of its three FK columns.
  it('passes on a valid construction_projects row', async () => {
    const seedFiles = {
      'agents.sql':
        "INSERT INTO agents (id, current_location_id) VALUES ('eddy_lin', 'lin_bedroom');\n",
      'locations.sql':
        "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      'construction_companies.sql':
        "INSERT INTO construction_companies (id) VALUES ('smallville_construction');\n",
      'construction_projects.sql':
        "INSERT INTO construction_projects (id, agent_id, company_id, location_id) " +
        "VALUES ('proj_1', 'eddy_lin', 'smallville_construction', 'lin_bedroom');\n",
      'seed-order.txt':
        'locations.sql\nagents.sql\nconstruction_companies.sql\nconstruction_projects.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const fkFailures = result.failures.filter(f => f.check === 'fk-ref');
      assert.deepStrictEqual(fkFailures, [],
        'Valid construction_projects row must not produce FK failures');
    });
  });

  // ── PARSER SAFETY ──────────────────────────────────────────────────

  it('parser: REFERENCES inside a -- comment does not create a rule', () => {
    const migs = {
      '0001.sql':
        'CREATE TABLE foo (\n' +
        '  id TEXT PRIMARY KEY,\n' +
        '  -- bar_id TEXT REFERENCES bars(id),  (commented out, must be ignored)\n' +
        '  real_id TEXT REFERENCES reals(id)\n' +
        ');\n',
    };
    return withTempMigrations(migs, (dir) => {
      const rules = deriveFkRules(dir);
      assert.ok(rules.some(r => r.column === 'real_id' && r.targetTable === 'reals'),
        'Real REFERENCES must be parsed');
      assert.ok(!rules.some(r => r.targetTable === 'bars'),
        'REFERENCES inside a -- comment should not produce a rule');
    });
  });

  it('parser: PRIMARY KEY REFERENCES is treated as non-null', () => {
    const migs = {
      '0001.sql':
        'CREATE TABLE citizen_runtime_state (\n' +
        '  agent_id TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE\n' +
        ');\n',
    };
    return withTempMigrations(migs, (dir) => {
      const rules = deriveFkRules(dir);
      const r = rules.find(x => x.sourceTable === 'citizen_runtime_state' && x.column === 'agent_id');
      assert.ok(r, 'PK FK column must be parsed');
      assert.strictEqual(r.nullable, false, 'PRIMARY KEY REFERENCES must be non-null');
    });
  });

  it('parser: NOT NULL vs plain REFERENCES nullability is inferred correctly', () => {
    const rules = deriveFkRules(REAL_MIGRATIONS_DIR);
    const get = (st, c) => rules.find(r => r.sourceTable === st && r.column === c);
    // Plain REFERENCES (no NOT NULL) => nullable
    for (const [st, c] of [
      ['inventory_items', 'held_by'],
      ['inventory_items', 'location_id'],
      ['jobs', 'employer_id'],
      ['shops', 'owner_id'],
      ['banks', 'updated_by'],
    ]) {
      assert.strictEqual(get(st, c)?.nullable, true, `${st}.${c} should be nullable`);
    }
    // NOT NULL REFERENCES => non-null
    for (const [st, c] of [
      ['agent_jobs', 'agent_id'],
      ['location_roles', 'location_id'],
      ['agents', 'current_location_id'],
    ]) {
      assert.strictEqual(get(st, c)?.nullable, false, `${st}.${c} should be non-null`);
    }
  });

  it('nullability: nullable FK column with explicit NULL passes', async () => {
    // agents.home_location_id is `TEXT REFERENCES locations(id)` (no NOT NULL),
    // so an explicit NULL is allowed and should not fail.
    const seedFiles = {
      'locations.sql': "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      'agents.sql':
        "INSERT INTO agents (id, current_location_id, home_location_id) " +
        "VALUES ('eddy_lin', 'lin_bedroom', NULL);\n",
      'seed-order.txt': 'locations.sql\nagents.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const fkFailures = result.failures.filter(f => f.check === 'fk-ref');
      assert.deepStrictEqual(fkFailures, [],
        'Explicit NULL in a nullable FK column must be allowed');
    });
  });

  it('nullability: NON-null FK column with explicit NULL fails', async () => {
    // agents.current_location_id is `TEXT NOT NULL REFERENCES locations(id)`,
    // so an explicit NULL violates the derived non-null constraint and must fail.
    const seedFiles = {
      'locations.sql': "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      'agents.sql':
        "INSERT INTO agents (id, current_location_id) VALUES ('eddy_lin', NULL);\n",
      'seed-order.txt': 'locations.sql\nagents.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' &&
        f.message.includes('current_location_id') &&
        /NULL/i.test(f.message));
      assert.ok(found,
        'Explicit NULL in a NOT NULL FK column should fail (derived nullability is enforced)');
    });
  });

  it('parser: ALTER TABLE ADD COLUMN REFERENCES is accumulated (jobs.employer_id)', () => {
    const rules = deriveFkRules(REAL_MIGRATIONS_DIR);
    // jobs.employer_id is added via ALTER TABLE in 0012_job_system.sql, not in
    // the CREATE TABLE for jobs — it must still be derived.
    assert.ok(
      rules.some(r => r.sourceTable === 'jobs' && r.column === 'employer_id' &&
                      r.targetTable === 'agents' && r.targetColumn === 'id'),
      'ALTER TABLE ADD COLUMN ... REFERENCES must be accumulated onto the base table');
  });

  it('identifiers: UPPERCASE unquoted table/column names normalize and still validate', async () => {
    // Seed uses uppercase identifiers; Postgres folds them to lowercase, so
    // the FK rule (agents.id) must still match a seeded value in AGENTS.ID.
    const seedFiles = {
      'locations.sql': "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      // Uppercase table + column identifiers; the bad value must still be caught.
      'agents.sql':
        "INSERT INTO AGENTS (ID, CURRENT_LOCATION_ID) VALUES ('eddy_lin', 'NOWHERE');\n",
      'seed-order.txt': 'locations.sql\nagents.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' && f.message.includes('NOWHERE'));
      assert.ok(found,
        'Uppercase identifiers should normalize so the FK rule still applies');
    });
  });

  it('values: string literal case is PRESERVED (Rosie_Kim != rosie_kim)', async () => {
    // The seeded agent id is 'rosie_kim'; a row references 'Rosie_Kim'.
    // Postgres TEXT comparison is case-sensitive, so this should fail —
    // identifier folding should not bleed into value comparison.
    const seedFiles = {
      'locations.sql': "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      'agents.sql':
        "INSERT INTO agents (id, current_location_id) VALUES ('rosie_kim', 'lin_bedroom');\n",
      // shops.owner_id -> agents.id, referencing a differently-cased value
      'shops.sql':
        "INSERT INTO shops (id, owner_id) VALUES ('shop_1', 'Rosie_Kim');\n",
      'seed-order.txt': 'locations.sql\nagents.sql\nshops.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' && f.message.includes('Rosie_Kim'));
      assert.ok(found,
        'FK value comparison must be case-sensitive: Rosie_Kim must not match seeded rosie_kim');
    });
  });

  it('keywords: lowercase SQL keywords are still parsed (regression guard)', async () => {
    const seedFiles = {
      'locations.sql': "insert into locations (id) values ('lin_bedroom');\n",
      'agents.sql':
        "insert into agents (id, current_location_id) values ('eddy_lin', 'NOWHERE');\n",
      'seed-order.txt': 'locations.sql\nagents.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' && f.message.includes('NOWHERE'));
      assert.ok(found,
        'Lowercase insert/values keywords must still be parsed so the FK check runs');
    });
  });

  // ── skip-vs-fail distinction ───────────────────────────────────────

  it('skips a rule whose target table.column is never seeded', async () => {
    // economy_transactions.from_agent_id -> agents.id exists in migrations,
    // but if agents is not seeded at all there is nothing to check against,
    // so the rule is skipped rather than failed.
    const seedFiles = {
      'economy_transactions.sql':
        "INSERT INTO economy_transactions (id, from_agent_id) VALUES ('tx_1', 'ghost_agent');\n",
      'seed-order.txt': 'economy_transactions.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const fkFailures = result.failures.filter(f => f.check === 'fk-ref');
      assert.deepStrictEqual(fkFailures, [],
        'A rule whose target is seeded nowhere should be skipped quietly');
    });
  });

  it('fails when the target is seeded only in a later file', async () => {
    // agents is seeded, but only after the file that references it. Tracking
    // which values exist anywhere should not relax the order-sensitive check;
    // a reference loaded too late should still fail.
    const seedFiles = {
      'locations.sql': "INSERT INTO locations (id) VALUES ('lin_bedroom');\n",
      'shops.sql':
        "INSERT INTO shops (id, owner_id) VALUES ('shop_1', 'eddy_lin');\n",
      'agents.sql':
        "INSERT INTO agents (id, current_location_id) VALUES ('eddy_lin', 'lin_bedroom');\n",
      // shops loads BEFORE agents -> eddy_lin not yet available when shop_1 refs it
      'seed-order.txt': 'locations.sql\nshops.sql\nagents.sql\n',
    };
    await withTempSeeds(seedFiles, async (tmp) => {
      const result = validateSeeds(tmp, { seedOrderPath: join(tmp, 'seed-order.txt') });
      const found = result.failures.some(f =>
        f.check === 'fk-ref' && f.message.includes('eddy_lin'));
      assert.ok(found,
        'A reference to a value seeded only in a later file should still fail');
    });
  });

});
