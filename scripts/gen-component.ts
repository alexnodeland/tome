#!/usr/bin/env npx ts-node

/**
 * Generate a new Svelte component with test file
 *
 * Usage: npm run gen:component ComponentName
 *
 * Creates:
 * - src/lib/components/ComponentName.svelte
 * - src/lib/components/ComponentName.test.ts
 * - Updates src/lib/components/index.ts
 */

import * as fs from 'fs';
import * as path from 'path';

const COMPONENT_TEMPLATE = `<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  // Props
  export let value: string = '';

  // Events
  const dispatch = createEventDispatcher<{
    change: string;
  }>();

  // Handlers
  function handleChange(newValue: string): void {
    dispatch('change', newValue);
  }
</script>

<div class="{{kebab}}">
  <slot />
</div>

<style>
  .{{kebab}} {
    /* Component styles using design system variables */
  }
</style>
`;

const TEST_TEMPLATE = `import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';

import {{name}} from './{{name}}.svelte';

describe('{{name}}', () => {
  it('renders correctly', () => {
    render({{name}});
    // Add your assertions
  });

  it('handles user interaction', async () => {
    const { component } = render({{name}});

    const handler = vi.fn();
    component.$on('change', handler);

    // Add interaction test
  });
});
`;

function toKebabCase(str: string): string {
  return str.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
}

function main(): void {
  const name = process.argv[2];

  if (!name) {
    console.error('Usage: npm run gen:component <ComponentName>');
    console.error('Example: npm run gen:component SearchResults');
    process.exit(1);
  }

  // Validate PascalCase
  if (!/^[A-Z][a-zA-Z0-9]+$/.test(name)) {
    console.error('Error: Component name must be PascalCase (e.g., SearchResults)');
    process.exit(1);
  }

  const kebab = toKebabCase(name);
  const componentsDir = path.join(__dirname, '../src/lib/components');

  // Ensure directory exists
  if (!fs.existsSync(componentsDir)) {
    fs.mkdirSync(componentsDir, { recursive: true });
  }

  // Check if component already exists
  const componentPath = path.join(componentsDir, `${name}.svelte`);
  if (fs.existsSync(componentPath)) {
    console.error(`Error: Component ${name} already exists at ${componentPath}`);
    process.exit(1);
  }

  // Write component file
  const componentContent = COMPONENT_TEMPLATE.replace(/\{\{name\}\}/g, name).replace(
    /\{\{kebab\}\}/g,
    kebab
  );
  fs.writeFileSync(componentPath, componentContent);
  console.log(`✓ Created: ${componentPath}`);

  // Write test file
  const testPath = path.join(componentsDir, `${name}.test.ts`);
  const testContent = TEST_TEMPLATE.replace(/\{\{name\}\}/g, name);
  fs.writeFileSync(testPath, testContent);
  console.log(`✓ Created: ${testPath}`);

  // Update index.ts
  const indexPath = path.join(componentsDir, 'index.ts');
  const exportLine = `export { default as ${name} } from './${name}.svelte';\n`;

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

  console.log(`\n✅ Component ${name} created successfully!`);
  console.log(`\nNext steps:`);
  console.log(`  1. Edit ${componentPath} to implement your component`);
  console.log(`  2. Write tests in ${testPath}`);
  console.log(`  3. Import with: import { ${name} } from '$lib/components';`);
}

main();
