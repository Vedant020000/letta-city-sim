#!/usr/bin/env node
// validate-seeds.mjs — Static seed-data validator for letta-city-sim
// Zero dependencies: Node 20 stdlib only.
//
// Checks:
//   1. column-count    — VALUES tuple count matches column list
//   2. double-quote    — flags "string" in VALUES (likely meant 'string')
//   3. jsonb           — parses every '...'::jsonb literal with JSON.parse
//   4. fk-ref          — order-aware FK reference check against seed-order.txt
//   5. adjacency-symmetry — every (A,B) edge has a (B,A) reverse
//   6. inventory-xor   — inventory_items: exactly one of held_by/location_id is non-NULL
//   7. consumable      — consumable_type must be in allowed set; vital_value/quantity > 0
//   8. seed-order      — every .sql in seed/ must be in seed-order.txt and vice versa

import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Strip SQL single-line comments (-- to EOL).
 * Preserves strings: does not strip inside '...' literals.
 */
function stripComments(sql) {
  const lines = sql.split('\n');
  const result = [];
  for (const line of lines) {
    let inStr = false;
    let out = '';
    for (let i = 0; i < line.length; i++) {
      const ch = line[i];
      if (inStr) {
        out += ch;
        if (ch === "'" && i + 1 < line.length && line[i + 1] === "'") {
          out += line[i + 1];
          i++;
        } else if (ch === "'") {
          inStr = false;
        }
      } else {
        if (ch === "'" ) {
          inStr = true;
          out += ch;
        } else if (ch === '-' && i + 1 < line.length && line[i + 1] === '-') {
          break; // rest of line is comment
        } else {
          out += ch;
        }
      }
    }
    result.push(out);
  }
  return result.join('\n');
}

/**
 * Parse a parenthesised column list: (col1, col2, ...) → ['col1','col2',...]
 */
function parseColumnList(str) {
  // str starts at '(' — find matching ')'
  let depth = 0;
  let start = -1;
  let end = -1;
  for (let i = 0; i < str.length; i++) {
    if (str[i] === '(') {
      if (depth === 0) start = i + 1;
      depth++;
    } else if (str[i] === ')') {
      depth--;
      if (depth === 0) { end = i; break; }
    }
  }
  if (start === -1 || end === -1) return [];
  const inner = str.slice(start, end);
  return inner.split(',').map(c => c.trim()).filter(Boolean);
}

/**
 * Tokenize a VALUES section into individual tuples.
 * Each tuple is the text between matching parens at depth 0.
 * Respects strings, nested parens, and ARRAY[...].
 */
function extractValuesTuples(valuesText) {
  const tuples = [];
  let depth = 0;
  let inStr = false;
  let tupleStart = -1;

  for (let i = 0; i < valuesText.length; i++) {
    const ch = valuesText[i];
    if (inStr) {
      if (ch === "'" && i + 1 < valuesText.length && valuesText[i + 1] === "'") {
        i++; // skip escaped quote
      } else if (ch === "'") {
        inStr = false;
      }
      continue;
    }
    if (ch === "'") {
      inStr = true;
      continue;
    }
    if (ch === '(') {
      if (depth === 0) tupleStart = i + 1;
      depth++;
    } else if (ch === ')') {
      depth--;
      if (depth === 0 && tupleStart !== -1) {
        tuples.push(valuesText.slice(tupleStart, i));
        tupleStart = -1;
      }
    }
  }
  return tuples;
}

/**
 * Count values in a single tuple string, respecting nested parens,
 * strings, and ARRAY[...] literals.
 * Returns the value strings as an array.
 */
function splitTupleValues(tupleText) {
  const values = [];
  let depth = 0; // parens
  let bracketDepth = 0; // brackets for ARRAY[...]
  let inStr = false;
  let current = '';

  for (let i = 0; i < tupleText.length; i++) {
    const ch = tupleText[i];
    if (inStr) {
      current += ch;
      if (ch === "'" && i + 1 < tupleText.length && tupleText[i + 1] === "'") {
        current += tupleText[i + 1];
        i++;
      } else if (ch === "'") {
        inStr = false;
      }
      continue;
    }
    if (ch === "'") {
      inStr = true;
      current += ch;
      continue;
    }
    if (ch === '(') { depth++; current += ch; continue; }
    if (ch === ')') { depth--; current += ch; continue; }
    if (ch === '[') { bracketDepth++; current += ch; continue; }
    if (ch === ']') { bracketDepth--; current += ch; continue; }
    if (ch === ',' && depth === 0 && bracketDepth === 0) {
      values.push(current.trim());
      current = '';
      continue;
    }
    current += ch;
  }
  if (current.trim()) values.push(current.trim());
  return values;
}

/**
 * Extract the unquoted string value from a SQL value like 'foo' or 'foo'::text.
 * Returns null for NULL literals.
 */
function unquoteValue(val) {
  const trimmed = val.trim();
  if (trimmed.toUpperCase() === 'NULL') return null;
  // Match 'content' possibly followed by ::type
  const m = trimmed.match(/^'((?:[^']|'')*)'(?:::[\w]+)?$/);
  if (m) return m[1].replace(/''/g, "'");
  // Unquoted number or boolean
  return trimmed;
}

/**
 * Find all INSERT INTO statements in a SQL file.
 * Returns array of { table, columns, tuples, lineNumber, fileName }.
 */
function parseInserts(sql, fileName) {
  const stripped = stripComments(sql);
  const inserts = [];

  // Regex to find INSERT INTO <table> (<columns>) VALUES
  // We need to handle multi-line carefully
  const insertRegex = /INSERT\s+INTO\s+(\w+)\s*\(([^)]+)\)\s*VALUES\b/gi;
  let match;

  while ((match = insertRegex.exec(stripped)) !== null) {
    const table = match[1];
    const columns = match[2].split(',').map(c => c.trim()).filter(Boolean);
    const afterValues = stripped.slice(match.index + match[0].length);

    // Find the end of the VALUES section: look for ON CONFLICT or ; at depth 0
    let depth = 0;
    let inStr = false;
    let end = afterValues.length;
    for (let i = 0; i < afterValues.length; i++) {
      const ch = afterValues[i];
      if (inStr) {
        if (ch === "'" && i + 1 < afterValues.length && afterValues[i + 1] === "'") {
          i++;
        } else if (ch === "'") {
          inStr = false;
        }
        continue;
      }
      if (ch === "'") { inStr = true; continue; }
      if (ch === '(') { depth++; continue; }
      if (ch === ')') {
        depth--;
        if (depth < 0) { end = i; break; }
        continue;
      }
      if (depth === 0 && ch === ';') { end = i; break; }
      // Check for ON CONFLICT at depth 0
      if (depth === 0) {
        const remaining = afterValues.slice(i);
        if (/^ON\s+CONFLICT\b/i.test(remaining)) { end = i; break; }
      }
    }

    const valuesText = afterValues.slice(0, end);
    const tuples = extractValuesTuples(valuesText);

    const lineNumber = (stripped.slice(0, match.index).match(/\n/g) || []).length + 1;

    inserts.push({ table, columns, tuples, lineNumber, fileName });
  }

  return inserts;
}

/**
 * Extract '...'::jsonb literals from SQL text.
 * Returns array of { json, lineApprox }.
 */
function extractJsonbLiterals(sql) {
  const results = [];
  const stripped = stripComments(sql);

  // Find '...'::jsonb patterns
  let i = 0;
  while (i < stripped.length) {
    if (stripped[i] === "'") {
      const start = i;
      i++;
      let content = '';
      while (i < stripped.length) {
        if (stripped[i] === "'" && i + 1 < stripped.length && stripped[i + 1] === "'") {
          content += "'";
          i += 2;
        } else if (stripped[i] === "'") {
          i++;
          break;
        } else {
          content += stripped[i];
          i++;
        }
      }
      // Check for ::jsonb after the closing quote
      const after = stripped.slice(i).match(/^::jsonb\b/i);
      if (after) {
        const lineApprox = (stripped.slice(0, start).match(/\n/g) || []).length + 1;
        results.push({ json: content, lineApprox });
        i += after[0].length;
      }
    } else {
      i++;
    }
  }
  return results;
}

/**
 * Extract "double-quoted strings" from VALUES tuples that look like
 * string mistakes (not column names, not inside ::jsonb casts).
 */
function findDoubleQuotedStrings(sql) {
  const results = [];
  const stripped = stripComments(sql);

  // Find VALUES sections
  const valuesRegex = /\bVALUES\b/gi;
  let match;
  while ((match = valuesRegex.exec(stripped)) !== null) {
    const afterValues = stripped.slice(match.index + match[0].length);
    // Scan for double-quoted strings in VALUE tuples
    // But skip anything inside '...'::jsonb
    let inSingleStr = false;
    let inJsonbCast = false;
    for (let i = 0; i < afterValues.length; i++) {
      const ch = afterValues[i];
      if (inSingleStr) {
        if (ch === "'" && i + 1 < afterValues.length && afterValues[i + 1] === "'") {
          i++;
        } else if (ch === "'") {
          inSingleStr = false;
          // Check if followed by ::jsonb
          const rest = afterValues.slice(i + 1);
          if (/^::jsonb\b/i.test(rest)) {
            inJsonbCast = false;
          }
        }
        continue;
      }
      if (ch === "'") {
        inSingleStr = true;
        continue;
      }
      // Check for ON CONFLICT or ; to stop scanning
      if (/^ON\s+CONFLICT\b/i.test(afterValues.slice(i)) || ch === ';') break;

      if (ch === '"') {
        // Found a double-quote inside VALUES — extract the string
        const dqStart = i;
        i++;
        let dqContent = '';
        while (i < afterValues.length && afterValues[i] !== '"') {
          dqContent += afterValues[i];
          i++;
        }
        // Only flag if it looks like a value (contains spaces or apostrophes)
        // and not a simple identifier
        if (dqContent.length > 0 && /[' ]/.test(dqContent)) {
          const lineApprox = (stripped.slice(0, match.index + match[0].length + dqStart).match(/\n/g) || []).length + 1;
          results.push({ content: dqContent, lineApprox });
        }
      }
    }
  }
  return results;
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

const ALLOWED_CONSUMABLE_TYPES = new Set([
  'food', 'water', 'stamina', 'sleep', 'hygiene', 'appearance'
]);

/**
 * Main validation function.
 * @param {string} seedDir — path to the seed/ directory
 * @param {object} opts — { seedOrderPath?: string }
 * @returns {{ failures: Array<{ check: string, file: string, line: number, message: string }> }}
 */
export async function validateSeeds(seedDir, opts = {}) {
  const seedOrderPath = opts.seedOrderPath
    ?? join(__dirname, 'seed-order.txt');

  const failures = [];

  function fail(check, file, line, message) {
    failures.push({ check, file, line, message });
  }

  // ── Read seed-order.txt ────────────────────────────────────────────────
  let seedOrderLines;
  try {
    const raw = readFileSync(seedOrderPath, 'utf-8');
    seedOrderLines = raw.split('\n')
      .map(l => l.trim())
      .filter(l => l && !l.startsWith('#'));
  } catch (e) {
    fail('seed-order', seedOrderPath, 0, `Cannot read seed-order.txt: ${e.message}`);
    return { failures };
  }

  // ── Check 8: seed-order enforcement ────────────────────────────────────
  const sqlFilesOnDisk = new Set(
    readdirSync(seedDir).filter(f => f.endsWith('.sql'))
  );
  const seedOrderSet = new Set(seedOrderLines);

  for (const diskFile of sqlFilesOnDisk) {
    if (!seedOrderSet.has(diskFile)) {
      fail('seed-order', diskFile, 0,
        `${diskFile} exists in seed/ but is not listed in seed-order.txt`);
    }
  }
  for (const orderedFile of seedOrderLines) {
    if (!sqlFilesOnDisk.has(orderedFile)) {
      fail('seed-order', orderedFile, 0,
        `${orderedFile} is listed in seed-order.txt but does not exist in seed/`);
    }
  }

  // If seed-order has fatal issues, we can still run other checks on files that exist
  // ── Read and parse all seed files ──────────────────────────────────────
  const fileContents = new Map(); // filename → raw SQL
  const fileInserts = new Map();  // filename → parsed inserts

  for (const fileName of seedOrderLines) {
    const filePath = join(seedDir, fileName);
    try {
      const sql = readFileSync(filePath, 'utf-8');
      fileContents.set(fileName, sql);
      fileInserts.set(fileName, parseInserts(sql, fileName));
    } catch {
      // Already flagged in seed-order check if missing
    }
  }

  // Also read files on disk that aren't in seed-order (for completeness)
  for (const diskFile of sqlFilesOnDisk) {
    if (!fileContents.has(diskFile)) {
      const filePath = join(seedDir, diskFile);
      try {
        const sql = readFileSync(filePath, 'utf-8');
        fileContents.set(diskFile, sql);
        fileInserts.set(diskFile, parseInserts(sql, diskFile));
      } catch { /* skip */ }
    }
  }

  // ── Check 1: column-count match ────────────────────────────────────────
  for (const [fileName, inserts] of fileInserts) {
    for (const ins of inserts) {
      const expectedCols = ins.columns.length;
      for (let ti = 0; ti < ins.tuples.length; ti++) {
        const values = splitTupleValues(ins.tuples[ti]);
        if (values.length !== expectedCols) {
          fail('column-count', fileName, ins.lineNumber,
            `${ins.table} row ${ti + 1}: expected ${expectedCols} columns, found ${values.length}`);
        }
      }
    }
  }

  // ── Check 2: double-quote string lint ──────────────────────────────────
  for (const [fileName, sql] of fileContents) {
    const dqs = findDoubleQuotedStrings(sql);
    for (const dq of dqs) {
      fail('double-quote', fileName, dq.lineApprox,
        `Suspicious double-quoted string: "${dq.content}" — did you mean '${dq.content}'?`);
    }
  }

  // ── Check 3: JSONB literal validation ──────────────────────────────────
  for (const [fileName, sql] of fileContents) {
    const jsonbs = extractJsonbLiterals(sql);
    for (const jb of jsonbs) {
      try {
        JSON.parse(jb.json);
      } catch (e) {
        fail('jsonb', fileName, jb.lineApprox,
          `Invalid JSON in ::jsonb literal: ${e.message} — content: ${jb.json.slice(0, 60)}`);
      }
    }
  }

  // ── Check 4: FK reference inventory (order-aware) ──────────────────────
  // Accumulating ID sets, built up as we process files in seed-order.
  // For each file: collect its own IDs first (so intra-file references
  // resolve), then check FK columns against IDs available from previous
  // files plus this file's own IDs, then merge into the accumulated sets.
  const idSets = {
    locations: new Set(),   // locations + dorms both insert into 'locations'
    agents: new Set(),
    jobs: new Set(),
  };

  // FK map: source table.column → target ID set, with nullable flag.
  // Nullable FKs (shops.owner_id, banks.updated_by, etc.) are only
  // checked when the value is non-NULL.
  const fkRules = [
    { sourceTable: 'location_adjacency', column: 'from_id', targetSet: 'locations' },
    { sourceTable: 'location_adjacency', column: 'to_id', targetSet: 'locations' },
    { sourceTable: 'world_objects', column: 'location_id', targetSet: 'locations' },
    { sourceTable: 'inventory_items', column: 'location_id', targetSet: 'locations', nullable: true },
    { sourceTable: 'inventory_items', column: 'held_by', targetSet: 'agents', nullable: true },
    { sourceTable: 'agents', column: 'current_location_id', targetSet: 'locations' },
    { sourceTable: 'agents', column: 'home_location_id', targetSet: 'locations', nullable: true },
    { sourceTable: 'jobs', column: 'employer_id', targetSet: 'agents', nullable: true },
    { sourceTable: 'agent_jobs', column: 'agent_id', targetSet: 'agents' },
    { sourceTable: 'agent_jobs', column: 'job_id', targetSet: 'jobs' },
    { sourceTable: 'location_roles', column: 'location_id', targetSet: 'locations' },
    { sourceTable: 'location_roles', column: 'agent_id', targetSet: 'agents' },
    { sourceTable: 'shops', column: 'owner_id', targetSet: 'agents', nullable: true },
    { sourceTable: 'shops', column: 'shopkeeper_job_id', targetSet: 'jobs', nullable: true },
    { sourceTable: 'banks', column: 'banker_job_id', targetSet: 'jobs', nullable: true },
    { sourceTable: 'banks', column: 'updated_by', targetSet: 'agents', nullable: true },
  ];

  for (const fileName of seedOrderLines) {
    const inserts = fileInserts.get(fileName);
    if (!inserts) continue;

    // Phase 1: collect all IDs this file introduces (so intra-file refs work)
    const fileIds = { locations: new Set(), agents: new Set(), jobs: new Set() };
    for (const ins of inserts) {
      const idSetKey =
        ins.table === 'locations' ? 'locations' :
        ins.table === 'agents' ? 'agents' :
        ins.table === 'jobs' ? 'jobs' :
        null;
      if (idSetKey) {
        for (const tuple of ins.tuples) {
          const values = splitTupleValues(tuple);
          if (values.length > 0) {
            const id = unquoteValue(values[0]);
            if (id !== null) fileIds[idSetKey].add(id);
          }
        }
      }
    }

    // Available IDs = accumulated from previous files + this file's own IDs
    const availableSets = {
      locations: new Set([...idSets.locations, ...fileIds.locations]),
      agents: new Set([...idSets.agents, ...fileIds.agents]),
      jobs: new Set([...idSets.jobs, ...fileIds.jobs]),
    };

    // Phase 2: check FK references against available IDs
    for (const ins of inserts) {
      for (const rule of fkRules) {
        if (rule.sourceTable !== ins.table) continue;
        const colIdx = ins.columns.indexOf(rule.column);
        if (colIdx === -1) continue;

        for (let ti = 0; ti < ins.tuples.length; ti++) {
          const values = splitTupleValues(ins.tuples[ti]);
          if (colIdx >= values.length) continue;
          const rawVal = unquoteValue(values[colIdx]);
          if (rawVal === null) continue; // NULL values skip FK check
          if (!availableSets[rule.targetSet].has(rawVal)) {
            fail('fk-ref', fileName, ins.lineNumber,
              `${ins.table} row ${ti + 1}: ${rule.column} '${rawVal}' not found in ${rule.targetSet} (loaded up to and including ${fileName})`);
          }
        }
      }
    }

    // Phase 3: merge this file's IDs into the accumulated sets
    for (const key of Object.keys(fileIds)) {
      for (const id of fileIds[key]) {
        idSets[key].add(id);
      }
    }
  }

  // ── Check 5: adjacency symmetry ────────────────────────────────────────
  const adjacencyEdges = new Set();
  const adjacencyPairs = [];

  for (const [fileName, inserts] of fileInserts) {
    for (const ins of inserts) {
      if (ins.table !== 'location_adjacency') continue;
      const fromIdx = ins.columns.indexOf('from_id');
      const toIdx = ins.columns.indexOf('to_id');
      if (fromIdx === -1 || toIdx === -1) continue;

      for (const tuple of ins.tuples) {
        const values = splitTupleValues(tuple);
        const fromId = unquoteValue(values[fromIdx]);
        const toId = unquoteValue(values[toIdx]);
        if (fromId && toId) {
          adjacencyEdges.add(`${fromId}->${toId}`);
          adjacencyPairs.push({ from: fromId, to: toId, fileName });
        }
      }
    }
  }

  for (const pair of adjacencyPairs) {
    const reverse = `${pair.to}->${pair.from}`;
    if (!adjacencyEdges.has(reverse)) {
      fail('adjacency-symmetry', pair.fileName, 0,
        `Edge (${pair.from} → ${pair.to}) has no reverse (${pair.to} → ${pair.from})`);
    }
  }

  // ── Check 6: inventory XOR constraint ──────────────────────────────────
  for (const [fileName, inserts] of fileInserts) {
    for (const ins of inserts) {
      if (ins.table !== 'inventory_items') continue;
      const heldByIdx = ins.columns.indexOf('held_by');
      const locationIdx = ins.columns.indexOf('location_id');
      if (heldByIdx === -1 || locationIdx === -1) continue;

      for (let ti = 0; ti < ins.tuples.length; ti++) {
        const values = splitTupleValues(ins.tuples[ti]);
        const heldBy = unquoteValue(values[heldByIdx]);
        const locationId = unquoteValue(values[locationIdx]);
        const heldByIsNull = heldBy === null;
        const locationIsNull = locationId === null;

        // XOR: exactly one must be non-NULL
        if (heldByIsNull === locationIsNull) {
          fail('inventory-xor', fileName, ins.lineNumber,
            `inventory_items row ${ti + 1}: ` +
            (heldByIsNull
              ? 'both held_by and location_id are NULL'
              : 'both held_by and location_id are set') +
            ' — exactly one must be non-NULL');
        }
      }
    }
  }

  // ── Check 7: consumable integrity ──────────────────────────────────────
  for (const [fileName, inserts] of fileInserts) {
    for (const ins of inserts) {
      if (ins.table !== 'inventory_items') continue;
      const ctIdx = ins.columns.indexOf('consumable_type');
      const vvIdx = ins.columns.indexOf('vital_value');
      const qtyIdx = ins.columns.indexOf('quantity');
      if (ctIdx === -1) continue;

      for (let ti = 0; ti < ins.tuples.length; ti++) {
        const values = splitTupleValues(ins.tuples[ti]);
        const consumableType = unquoteValue(values[ctIdx]);
        if (consumableType === null) continue; // non-consumable item

        if (!ALLOWED_CONSUMABLE_TYPES.has(consumableType)) {
          fail('consumable', fileName, ins.lineNumber,
            `inventory_items row ${ti + 1}: consumable_type '${consumableType}' not in allowed set {${[...ALLOWED_CONSUMABLE_TYPES].join(', ')}}`);
        }

        if (vvIdx !== -1 && vvIdx < values.length) {
          const vitalValue = unquoteValue(values[vvIdx]);
          if (vitalValue === null || isNaN(Number(vitalValue)) || Number(vitalValue) <= 0) {
            fail('consumable', fileName, ins.lineNumber,
              `inventory_items row ${ti + 1}: vital_value must be a positive integer, got '${vitalValue}'`);
          }
        }

        if (qtyIdx !== -1 && qtyIdx < values.length) {
          const quantity = unquoteValue(values[qtyIdx]);
          if (quantity === null || isNaN(Number(quantity)) || Number(quantity) <= 0) {
            fail('consumable', fileName, ins.lineNumber,
              `inventory_items row ${ti + 1}: quantity must be a positive integer, got '${quantity}'`);
          }
        }
      }
    }
  }

  // ── Summary ────────────────────────────────────────────────────────────
  if (failures.length === 0) {
    // Print passing summary
    const totalEdges = adjacencyPairs.length;
    const totalFiles = sqlFilesOnDisk.size;
    console.log(`PASS  [all-checks]  ${totalFiles} seed files, ${totalEdges} adjacency edges, all checks passed`);
  }

  return { failures };
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

const isMain = process.argv[1] &&
  fileURLToPath(import.meta.url).replace(/\\/g, '/') ===
  process.argv[1].replace(/\\/g, '/');

if (isMain) {
  const seedDir = join(__dirname, '..', 'seed');
  const result = await validateSeeds(seedDir);

  for (const f of result.failures) {
    console.log(`FAIL  [${f.check}]  ${f.file}:${f.line}  ${f.message}`);
  }

  if (result.failures.length > 0) {
    console.log(`\n${result.failures.length} failure(s) found.`);
    process.exit(1);
  } else {
    console.log('\nSeed-data validation passed (strong merge safety signal).');
    process.exit(0);
  }
}
