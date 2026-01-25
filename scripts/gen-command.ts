#!/usr/bin/env npx ts-node

/**
 * Generate a new Tauri command with service wrapper and tests
 *
 * Usage: npm run gen:command command_name
 *
 * Creates:
 * - src-tauri/src/commands/command_name.rs (stub)
 * - src/lib/services/command-name.ts
 * - src/lib/services/command-name.test.ts
 * - Updates mod.rs exports
 */

import * as fs from 'fs';
import * as path from 'path';

const RUST_TEMPLATE = `//! {{snake}} command handler

use tauri::{command, State};
use crate::AppState;

/// {{description}}
///
/// # Arguments
/// * \`state\` - Application state
///
/// # Errors
/// Returns error string if operation fails
#[command]
pub async fn {{snake}}(
    state: State<'_, AppState>,
) -> Result<String, String> {
    // TODO: Implement command logic
    Ok("Not implemented".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_{{snake}}() {
        // TODO: Add tests
    }
}
`;

const SERVICE_TEMPLATE = `/**
 * {{pascal}} service - Tauri command wrapper
 */

import { invoke } from '@tauri-apps/api/tauri';

import type { TomeResult, TomeError } from '$lib/types';

/**
 * {{description}}
 */
export async function {{camel}}(): Promise<TomeResult<string>> {
  try {
    const result = await invoke<string>('{{snake}}');
    return { ok: true, value: result };
  } catch (error) {
    return {
      ok: false,
      error: parseError(error),
    };
  }
}

function parseError(error: unknown): TomeError {
  if (typeof error === 'string') {
    return { code: 'UNKNOWN', message: error };
  }
  if (error instanceof Error) {
    return { code: 'UNKNOWN', message: error.message };
  }
  return { code: 'UNKNOWN', message: 'An unknown error occurred' };
}
`;

const TEST_TEMPLATE = `import { vi, describe, it, expect, beforeEach } from 'vitest';

import { {{camel}} } from './{{kebab}}';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/tauri';

describe('{{camel}} service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns success on valid response', async () => {
    vi.mocked(invoke).mockResolvedValue('success');

    const result = await {{camel}}();

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value).toBe('success');
    }
    expect(invoke).toHaveBeenCalledWith('{{snake}}');
  });

  it('returns error on failure', async () => {
    vi.mocked(invoke).mockRejectedValue('Operation failed');

    const result = await {{camel}}();

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('UNKNOWN');
      expect(result.error.message).toContain('failed');
    }
  });
});
`;

function toKebabCase(str: string): string {
  return str.replace(/_/g, '-');
}

function toPascalCase(str: string): string {
  return str
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join('');
}

function toCamelCase(str: string): string {
  const pascal = toPascalCase(str);
  return pascal.charAt(0).toLowerCase() + pascal.slice(1);
}

function main(): void {
  const name = process.argv[2];

  if (!name) {
    console.error('Usage: npm run gen:command <command_name>');
    console.error('Example: npm run gen:command sync_source');
    process.exit(1);
  }

  // Validate snake_case
  if (!/^[a-z][a-z0-9_]*$/.test(name)) {
    console.error('Error: Command name must be snake_case (e.g., sync_source)');
    process.exit(1);
  }

  const snake = name;
  const kebab = toKebabCase(name);
  const pascal = toPascalCase(name);
  const camel = toCamelCase(name);
  const description = pascal.replace(/([A-Z])/g, ' $1').trim();

  // Create Rust command stub
  const rustCommandsDir = path.join(__dirname, '../src-tauri/src/commands');
  const rustPath = path.join(rustCommandsDir, `${snake}.rs`);

  if (!fs.existsSync(rustCommandsDir)) {
    fs.mkdirSync(rustCommandsDir, { recursive: true });
  }

  if (!fs.existsSync(rustPath)) {
    const rustContent = RUST_TEMPLATE.replace(/\{\{snake\}\}/g, snake).replace(
      /\{\{description\}\}/g,
      description
    );
    fs.writeFileSync(rustPath, rustContent);
    console.log(`✓ Created: ${rustPath}`);
    console.log(`  ⚠️  Remember to add 'mod ${snake};' to commands/mod.rs`);
    console.log(`  ⚠️  Remember to add '${snake}' to generate_handler![] in main.rs`);
  } else {
    console.log(`⊘ Skipped (exists): ${rustPath}`);
  }

  // Create TypeScript service
  const servicesDir = path.join(__dirname, '../src/lib/services');
  const servicePath = path.join(servicesDir, `${kebab}.ts`);

  if (!fs.existsSync(servicesDir)) {
    fs.mkdirSync(servicesDir, { recursive: true });
  }

  if (!fs.existsSync(servicePath)) {
    const serviceContent = SERVICE_TEMPLATE.replace(/\{\{snake\}\}/g, snake)
      .replace(/\{\{camel\}\}/g, camel)
      .replace(/\{\{pascal\}\}/g, pascal)
      .replace(/\{\{description\}\}/g, description);
    fs.writeFileSync(servicePath, serviceContent);
    console.log(`✓ Created: ${servicePath}`);
  } else {
    console.log(`⊘ Skipped (exists): ${servicePath}`);
  }

  // Create test file
  const testPath = path.join(servicesDir, `${kebab}.test.ts`);
  if (!fs.existsSync(testPath)) {
    const testContent = TEST_TEMPLATE.replace(/\{\{snake\}\}/g, snake)
      .replace(/\{\{camel\}\}/g, camel)
      .replace(/\{\{kebab\}\}/g, kebab);
    fs.writeFileSync(testPath, testContent);
    console.log(`✓ Created: ${testPath}`);
  } else {
    console.log(`⊘ Skipped (exists): ${testPath}`);
  }

  // Update services index.ts
  const indexPath = path.join(servicesDir, 'index.ts');
  const exportLine = `export * from './${kebab}';\n`;

  if (fs.existsSync(indexPath)) {
    const existing = fs.readFileSync(indexPath, 'utf-8');
    if (!existing.includes(exportLine.trim())) {
      fs.appendFileSync(indexPath, exportLine);
      console.log(`✓ Updated: ${indexPath}`);
    }
  } else {
    fs.writeFileSync(indexPath, exportLine);
    console.log(`✓ Created: ${indexPath}`);
  }

  console.log(`\n✅ Command ${name} scaffolded successfully!`);
  console.log(`\nNext steps:`);
  console.log(`  1. Implement the Rust handler in ${rustPath}`);
  console.log(`  2. Add 'mod ${snake};' to src-tauri/src/commands/mod.rs`);
  console.log(`  3. Add '${snake}' to generate_handler![] in main.rs`);
  console.log(`  4. Customize the TypeScript service in ${servicePath}`);
  console.log(`  5. Write tests`);
}

main();
