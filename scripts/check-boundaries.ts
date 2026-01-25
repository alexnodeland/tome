#!/usr/bin/env npx ts-node

/**
 * Architecture Boundary Checker
 *
 * Validates that imports between directories follow the rules
 * defined in CLAUDE.md.
 *
 * Usage: npm run check:boundaries
 */

import * as fs from 'fs';
import * as path from 'path';

interface BoundaryRule {
  from: string;
  cannotImportFrom: string[];
}

// Import rules based on CLAUDE.md
const RULES: BoundaryRule[] = [
  {
    from: 'src/lib/utils',
    cannotImportFrom: ['src/lib/components', 'src/lib/stores', 'src/lib/services', 'src/routes'],
  },
  {
    from: 'src/lib/types',
    cannotImportFrom: [
      'src/lib/components',
      'src/lib/stores',
      'src/lib/services',
      'src/lib/utils',
      'src/routes',
    ],
  },
  {
    from: 'src/lib/services',
    cannotImportFrom: ['src/lib/components', 'src/lib/stores', 'src/routes'],
  },
  {
    from: 'src/lib/stores',
    cannotImportFrom: ['src/lib/components', 'src/routes'],
  },
  {
    from: 'src/lib/components',
    cannotImportFrom: ['src/routes'],
  },
];

interface Violation {
  file: string;
  line: number;
  importPath: string;
  rule: string;
}

function findFiles(dir: string, extensions: string[]): string[] {
  const files: string[] = [];

  if (!fs.existsSync(dir)) {
    return files;
  }

  function walk(currentDir: string): void {
    const entries = fs.readdirSync(currentDir, { withFileTypes: true });

    for (const entry of entries) {
      const fullPath = path.join(currentDir, entry.name);

      if (entry.isDirectory()) {
        // Skip node_modules and other irrelevant directories
        if (!['node_modules', 'dist', '.svelte-kit', 'coverage'].includes(entry.name)) {
          walk(fullPath);
        }
      } else if (extensions.some((ext) => entry.name.endsWith(ext))) {
        files.push(fullPath);
      }
    }
  }

  walk(dir);
  return files;
}

function extractImports(content: string): Array<{ line: number; path: string }> {
  const imports: Array<{ line: number; path: string }> = [];
  const lines = content.split('\n');

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Match various import patterns
    // import X from 'Y'
    // import { X } from 'Y'
    // import * as X from 'Y'
    // import 'Y'
    const importMatch = line?.match(/import\s+.*?from\s+['"]([^'"]+)['"]/);
    if (importMatch?.[1]) {
      imports.push({ line: i + 1, path: importMatch[1] });
    }

    // Dynamic imports: import('Y')
    const dynamicMatch = line?.match(/import\s*\(\s*['"]([^'"]+)['"]\s*\)/);
    if (dynamicMatch?.[1]) {
      imports.push({ line: i + 1, path: dynamicMatch[1] });
    }
  }

  return imports;
}

function normalizeImportPath(importPath: string, fromFile: string): string | null {
  // Handle $lib alias
  if (importPath.startsWith('$lib/')) {
    return 'src/lib/' + importPath.slice(5);
  }

  // Handle relative imports
  if (importPath.startsWith('.')) {
    const fileDir = path.dirname(fromFile);
    const resolved = path.normalize(path.join(fileDir, importPath));
    // Convert to forward slashes for consistency
    return resolved.replace(/\\/g, '/');
  }

  // External packages - not relevant for boundary checking
  return null;
}

function checkFile(filePath: string): Violation[] {
  const violations: Violation[] = [];

  // Find which rule applies to this file
  const relativePath = filePath.replace(/\\/g, '/');
  const rule = RULES.find((r) => relativePath.includes(r.from));

  if (!rule) {
    // No rule applies to this file
    return violations;
  }

  const content = fs.readFileSync(filePath, 'utf-8');
  const imports = extractImports(content);

  for (const imp of imports) {
    const normalizedPath = normalizeImportPath(imp.path, relativePath);

    if (!normalizedPath) {
      continue; // External import, skip
    }

    for (const forbidden of rule.cannotImportFrom) {
      if (normalizedPath.includes(forbidden)) {
        violations.push({
          file: relativePath,
          line: imp.line,
          importPath: imp.path,
          rule: `${rule.from} cannot import from ${forbidden}`,
        });
      }
    }
  }

  return violations;
}

function main(): void {
  const srcDir = path.join(__dirname, '../src');
  const files = findFiles(srcDir, ['.ts', '.svelte']);

  console.log(`Checking ${files.length} files for architecture boundary violations...\n`);

  const allViolations: Violation[] = [];

  for (const file of files) {
    const violations = checkFile(file);
    allViolations.push(...violations);
  }

  if (allViolations.length === 0) {
    console.log('✅ No architecture boundary violations found!\n');
    process.exit(0);
  }

  console.log('❌ Architecture boundary violations found:\n');

  for (const v of allViolations) {
    console.log(`  ${v.file}:${v.line}`);
    console.log(`    Import: ${v.importPath}`);
    console.log(`    Rule: ${v.rule}`);
    console.log();
  }

  console.log(`Total: ${allViolations.length} violation(s)\n`);
  console.log('See CLAUDE.md for module boundary rules.');
  process.exit(1);
}

main();
