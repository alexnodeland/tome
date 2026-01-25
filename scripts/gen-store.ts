#!/usr/bin/env npx ts-node

/**
 * Generate a new Svelte store with test file
 *
 * Usage: npm run gen:store storeName
 *
 * Creates:
 * - src/lib/stores/store-name.ts
 * - src/lib/stores/store-name.test.ts
 * - Updates src/lib/stores/index.ts
 */

import * as fs from 'fs';
import * as path from 'path';

const STORE_TEMPLATE = `import { writable, derived, type Readable } from 'svelte/store';

// === Types ===

export interface {{PascalName}}Item {
  id: string;
  // Add your fields here
}

// === Private State ===

const _items = writable<{{PascalName}}Item[]>([]);
const _loading = writable(false);
const _error = writable<string | null>(null);

// === Public Readable State ===

export const {{camelName}}s: Readable<{{PascalName}}Item[]> = { subscribe: _items.subscribe };
export const {{camelName}}Loading: Readable<boolean> = { subscribe: _loading.subscribe };
export const {{camelName}}Error: Readable<string | null> = { subscribe: _error.subscribe };

// === Derived State ===

export const {{camelName}}Count = derived(_items, ($items) => $items.length);

// === Actions ===

export const {{camelName}}Store = {
  /**
   * Load items from backend
   */
  async load(): Promise<void> {
    _loading.set(true);
    _error.set(null);
    try {
      // TODO: Call your service
      // const result = await listItems();
      // if (result.ok) {
      //   _items.set(result.value);
      // } else {
      //   _error.set(result.error.message);
      // }
    } finally {
      _loading.set(false);
    }
  },

  /**
   * Add an item
   */
  add(item: {{PascalName}}Item): void {
    _items.update((current) => [...current, item]);
  },

  /**
   * Remove an item by ID
   */
  remove(id: string): void {
    _items.update((current) => current.filter((item) => item.id !== id));
  },

  /**
   * Update an item
   */
  update(id: string, updates: Partial<{{PascalName}}Item>): void {
    _items.update((current) =>
      current.map((item) => (item.id === id ? { ...item, ...updates } : item))
    );
  },

  /**
   * Clear all items (useful for testing)
   */
  clear(): void {
    _items.set([]);
    _error.set(null);
  },
};
`;

const TEST_TEMPLATE = `import { get } from 'svelte/store';
import { describe, it, expect, beforeEach } from 'vitest';

import {
  {{camelName}}s,
  {{camelName}}Count,
  {{camelName}}Store,
  type {{PascalName}}Item,
} from './{{kebab}}';

describe('{{camelName}} store', () => {
  beforeEach(() => {
    {{camelName}}Store.clear();
  });

  it('starts with empty state', () => {
    expect(get({{camelName}}s)).toEqual([]);
    expect(get({{camelName}}Count)).toBe(0);
  });

  it('adds an item', () => {
    const item: {{PascalName}}Item = { id: '1' };
    {{camelName}}Store.add(item);

    expect(get({{camelName}}s)).toContainEqual(item);
    expect(get({{camelName}}Count)).toBe(1);
  });

  it('removes an item', () => {
    const item: {{PascalName}}Item = { id: '1' };
    {{camelName}}Store.add(item);
    {{camelName}}Store.remove('1');

    expect(get({{camelName}}s)).toEqual([]);
    expect(get({{camelName}}Count)).toBe(0);
  });

  it('updates an item', () => {
    const item: {{PascalName}}Item = { id: '1' };
    {{camelName}}Store.add(item);
    {{camelName}}Store.update('1', { /* your updates */ });

    const updated = get({{camelName}}s).find((i) => i.id === '1');
    expect(updated).toBeDefined();
    // Add your update assertions
  });

  it('clears all items', () => {
    {{camelName}}Store.add({ id: '1' });
    {{camelName}}Store.add({ id: '2' });
    {{camelName}}Store.clear();

    expect(get({{camelName}}s)).toEqual([]);
  });
});
`;

function toKebabCase(str: string): string {
  return str.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
}

function toPascalCase(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

function toCamelCase(str: string): string {
  return str.charAt(0).toLowerCase() + str.slice(1);
}

function main(): void {
  const name = process.argv[2];

  if (!name) {
    console.error('Usage: npm run gen:store <storeName>');
    console.error('Example: npm run gen:store bookmark');
    process.exit(1);
  }

  // Validate camelCase or simple name
  if (!/^[a-z][a-zA-Z0-9]*$/.test(name)) {
    console.error('Error: Store name should be camelCase (e.g., bookmark, navigationHistory)');
    process.exit(1);
  }

  const kebab = toKebabCase(name);
  const pascal = toPascalCase(name);
  const camel = toCamelCase(name);
  const storesDir = path.join(__dirname, '../src/lib/stores');

  // Ensure directory exists
  if (!fs.existsSync(storesDir)) {
    fs.mkdirSync(storesDir, { recursive: true });
  }

  // Check if store already exists
  const storePath = path.join(storesDir, `${kebab}.ts`);
  if (fs.existsSync(storePath)) {
    console.error(`Error: Store ${name} already exists at ${storePath}`);
    process.exit(1);
  }

  // Write store file
  const storeContent = STORE_TEMPLATE.replace(/\{\{PascalName\}\}/g, pascal).replace(
    /\{\{camelName\}\}/g,
    camel
  );
  fs.writeFileSync(storePath, storeContent);
  console.log(`✓ Created: ${storePath}`);

  // Write test file
  const testPath = path.join(storesDir, `${kebab}.test.ts`);
  const testContent = TEST_TEMPLATE.replace(/\{\{PascalName\}\}/g, pascal)
    .replace(/\{\{camelName\}\}/g, camel)
    .replace(/\{\{kebab\}\}/g, kebab);
  fs.writeFileSync(testPath, testContent);
  console.log(`✓ Created: ${testPath}`);

  // Update index.ts
  const indexPath = path.join(storesDir, 'index.ts');
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

  console.log(`\n✅ Store ${name} created successfully!`);
  console.log(`\nNext steps:`);
  console.log(`  1. Edit ${storePath} to add your item fields`);
  console.log(`  2. Implement the load() function with your service`);
  console.log(`  3. Write tests in ${testPath}`);
  console.log(`  4. Import with: import { ${camel}s, ${camel}Store } from '$lib/stores';`);
}

main();
