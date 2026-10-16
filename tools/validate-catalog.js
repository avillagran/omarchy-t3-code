#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');

function parseArgs(argv) {
  const options = { protect: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--source' || arg === '--catalog' || arg === '--protect') {
      const value = argv[++i];
      if (!value) throw new Error(`Missing value after ${arg}`);
      if (arg === '--protect') options.protect.push(value);
      else options[arg.slice(2)] = value;
    } else if (arg === '--help' || arg === '-h') options.help = true;
    else throw new Error(`Unknown argument: ${arg}`);
  }
  return options;
}

function readJson(filename, label, errors) {
  try {
    return JSON.parse(fs.readFileSync(filename, 'utf8'));
  } catch (error) {
    errors.push(`${label}: cannot read/parse ${filename}: ${error.message}`);
    return null;
  }
}

function placeholders(text) {
  const counts = new Map();
  for (const match of text.matchAll(/\{([A-Za-z_][A-Za-z0-9_]*)\}/g)) {
    counts.set(match[1], (counts.get(match[1]) || 0) + 1);
  }
  return counts;
}

// Conservative heuristic: commands/flags, absolute/home paths, URLs, and
// environment-variable references. It intentionally prefers warning over
// claiming a prose token is machine-significant.
function protectedTokens(text, extra = []) {
  const pattern = /(?:https?:\/\/[^\s"'`<>]+|(?<![\w])(?:\$HOME|~)?\/[A-Za-z0-9_.<][A-Za-z0-9_./<>{}$~@%+=-]*|\$(?:\{[A-Z_][A-Z0-9_]*\}|[A-Z_][A-Z0-9_]*)|(?<![^\s`"'])--?[A-Za-z][A-Za-z0-9-]*|\b[A-Z][A-Z_]{2,}(?=s?\b)|\b[a-zA-Z0-9_.+-]+\.(?:json|toml|qml|sh|conf|lua)\b)/g;
  const found = new Set((text.match(pattern) || []).map(token => token.replace(/[.,!?]+$/, '')));
  // Inline code is explicit author evidence, unlike hyphenated prose.
  for (const match of text.matchAll(/`([^`]+)`/g)) found.add(match[1]);
  for (const token of extra) if (text.includes(token)) found.add(token);
  return found;
}

function validate(source, catalog, extraProtected = []) {
  const errors = [];
  const warnings = [];
  if (!source || typeof source !== 'object' || Array.isArray(source)) {
    errors.push('Source must be a JSON object of locale objects.');
    return { errors, warnings };
  }
  if (!catalog || typeof catalog !== 'object' || Array.isArray(catalog)) {
    errors.push('Catalog must be a JSON object of locale objects.');
    return { errors, warnings };
  }
  const sourceLocales = Object.keys(source);
  if (!sourceLocales.length) errors.push('Source has no locale entries.');
  const sourceLocale = source.en ? 'en' : sourceLocales[0];
  const sourceMap = source[sourceLocale];
  if (!sourceMap || typeof sourceMap !== 'object' || Array.isArray(sourceMap)) {
    errors.push(`Source locale "${sourceLocale}" must map strings to strings.`);
    return { errors, warnings };
  }
  for (const [key, value] of Object.entries(sourceMap)) {
    if (typeof key !== 'string' || typeof value !== 'string') errors.push(`Source entry ${JSON.stringify(key)} must have a string value.`);
  }
  const sourceKeys = Object.keys(sourceMap);
  const sourceKeySet = new Set(sourceKeys);
  const sourceLocaleTag = /^[a-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$/;
  for (const [locale, entries] of Object.entries(catalog)) {
    if (!sourceLocaleTag.test(locale)) errors.push(`Locale "${locale}" should be a language tag such as "es" or "pt-BR".`);
    if (!entries || typeof entries !== 'object' || Array.isArray(entries)) {
      errors.push(`Locale "${locale}" must contain a flat object of source strings to translated strings.`);
      continue;
    }
    const present = new Set(Object.keys(entries));
    for (const [key, value] of Object.entries(entries)) {
      if (!sourceKeySet.has(key)) errors.push(`[${locale}] unknown source key: ${JSON.stringify(key)}`);
      if (typeof value !== 'string') errors.push(`[${locale}] ${JSON.stringify(key)}: translation must be a string.`);
      else if (!value.trim()) warnings.push(`[${locale}] ${JSON.stringify(key)}: empty translation falls back to source.`);
      else {
        const expected = placeholders(key);
        const actual = placeholders(value);
        if (JSON.stringify([...expected].sort()) !== JSON.stringify([...actual].sort())) {
          errors.push(`[${locale}] ${JSON.stringify(key)}: placeholder mismatch; expected ${JSON.stringify(Object.fromEntries(expected))}, got ${JSON.stringify(Object.fromEntries(actual))}.`);
        }
        const protectedSource = protectedTokens(key, extraProtected);
        const protectedTranslation = protectedTokens(value, extraProtected);
        for (const token of protectedSource) {
          if (!protectedTranslation.has(token)) {
            // A capitalized prose acronym may be localized (CPU -> ЦП).
            // Explicit protection or inline code makes its identity normative.
            const advisory = /^[A-Z][A-Z_]{2,}$/.test(token) && !extraProtected.includes(token) && !key.includes('`' + token + '`');
            const message = `[${locale}] ${JSON.stringify(key)}: ${advisory ? 'possible localized acronym' : 'protected token'} ${JSON.stringify(token)} is missing or changed.`;
            (advisory ? warnings : errors).push(message);
          }
        }
        for (const token of protectedTranslation) {
          if (!protectedSource.has(token)) warnings.push(`[${locale}] ${JSON.stringify(key)}: possible altered/additional protected token ${JSON.stringify(token)}; review manually.`);
        }
      }
    }
    for (const key of sourceKeys) if (!present.has(key)) warnings.push(`[${locale}] missing source key: ${JSON.stringify(key)}`);
  }
  return { errors, warnings, sourceLocale, sourceKeys: sourceKeys.length, locales: Object.keys(catalog).length };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write('Usage: node tools/validate-catalog.js --source SOURCE.json --catalog CATALOG.json [--protect TOKEN]\n');
    return 0;
  }
  if (!options.source || !options.catalog) throw new Error('Both --source and --catalog are required.');
  const errors = [];
  const source = readJson(options.source, 'source', errors);
  const catalog = readJson(options.catalog, 'catalog', errors);
  if (errors.length) {
    for (const error of errors) process.stderr.write(`ERROR: ${error}\n`);
    return 1;
  }
  const result = validate(source, catalog, options.protect);
  for (const warning of result.warnings) process.stdout.write(`WARN: ${warning}\n`);
  for (const error of result.errors) process.stderr.write(`ERROR: ${error}\n`);
  if (result.errors.length) return 1;
  process.stdout.write(`OK: ${result.locales} locale(s), ${result.sourceKeys} source key(s), source locale ${result.sourceLocale}.\n`);
  if (result.warnings.length) process.stdout.write(`${result.warnings.length} warning(s); review coverage and protected-token notes.\n`);
  return 0;
}

if (require.main === module) {
  try { process.exitCode = main(); }
  catch (error) { process.stderr.write(`ERROR: ${error.message}\n`); process.exitCode = 1; }
}
module.exports = { placeholders, protectedTokens, validate };
