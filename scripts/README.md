# Scripts Directory

Development scripts and code generators.

## What Belongs Here

- **Setup scripts** for development environment
- **Code generators** for components, commands, stores
- **Build helpers** and utilities
- **Validation scripts** for architecture enforcement

## What Does NOT Belong Here

- Application code
- Tests (use appropriate test directories)
- CI/CD configuration (use `.github/workflows/`)

## Available Scripts

### setup.sh

First-time setup script for new developers.

```bash
# Usage
./scripts/setup.sh

# What it does:
# 1. Checks prerequisites (Rust, Node, Xcode CLI)
# 2. Installs Rust toolchain and components
# 3. Installs npm dependencies
# 4. Builds the project
# 5. Runs initial test suite
# 6. Sets up pre-commit hooks
```

### gen-component.ts

Generate a new Svelte component with tests.

```bash
# Usage
npm run gen:component ComponentName

# Creates:
# - src/lib/components/ComponentName.svelte
# - src/lib/components/ComponentName.test.ts
# - Updates src/lib/components/index.ts
```

### gen-command.ts

Generate a new Tauri command with tests.

```bash
# Usage
npm run gen:command command_name

# Creates:
# - src-tauri/src/commands/command_name.rs
# - Updates src-tauri/src/commands/mod.rs
# - src/lib/services/command-name.ts
# - src/lib/services/command-name.test.ts
```

### gen-store.ts

Generate a new Svelte store with tests.

```bash
# Usage
npm run gen:store storeName

# Creates:
# - src/lib/stores/store-name.ts
# - src/lib/stores/store-name.test.ts
# - Updates src/lib/stores/index.ts
```

### check-boundaries.ts

Validate architectural boundaries.

```bash
# Usage
npm run check:boundaries

# Validates:
# - Import rules between directories
# - Circular dependencies
# - Module boundary violations
```

## Script Implementation Pattern

```typescript
// gen-component.ts
#!/usr/bin/env npx ts-node

import * as fs from 'fs';
import * as path from 'path';

const COMPONENT_TEMPLATE = `<script lang="ts">
  // Props
  export let prop: string = '';
</script>

<div class="{{kebab}}">
  {prop}
</div>

<style>
  .{{kebab}} {
    /* Component styles */
  }
</style>
`;

const TEST_TEMPLATE = `import { render, screen } from '@testing-library/svelte';
import {{name}} from './{{name}}.svelte';

describe('{{name}}', () => {
  it('renders correctly', () => {
    render({{name}}, { props: { prop: 'test' } });
    expect(screen.getByText('test')).toBeInTheDocument();
  });
});
`;

function main() {
  const name = process.argv[2];

  if (!name) {
    console.error('Usage: gen-component.ts <ComponentName>');
    process.exit(1);
  }

  if (!/^[A-Z][a-zA-Z0-9]+$/.test(name)) {
    console.error('Component name must be PascalCase');
    process.exit(1);
  }

  const kebab = name.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
  const componentsDir = path.join(__dirname, '../src/lib/components');

  // Check if already exists
  const componentPath = path.join(componentsDir, `${name}.svelte`);
  if (fs.existsSync(componentPath)) {
    console.error(`Component ${name} already exists`);
    process.exit(1);
  }

  // Write component
  const componentContent = COMPONENT_TEMPLATE
    .replace(/\{\{name\}\}/g, name)
    .replace(/\{\{kebab\}\}/g, kebab);
  fs.writeFileSync(componentPath, componentContent);
  console.log(`Created: ${componentPath}`);

  // Write test
  const testPath = path.join(componentsDir, `${name}.test.ts`);
  const testContent = TEST_TEMPLATE.replace(/\{\{name\}\}/g, name);
  fs.writeFileSync(testPath, testContent);
  console.log(`Created: ${testPath}`);

  // Update index.ts
  const indexPath = path.join(componentsDir, 'index.ts');
  const exportLine = `export { default as ${name} } from './${name}.svelte';\n`;

  if (fs.existsSync(indexPath)) {
    fs.appendFileSync(indexPath, exportLine);
  } else {
    fs.writeFileSync(indexPath, exportLine);
  }
  console.log(`Updated: ${indexPath}`);

  console.log(`\n✓ Component ${name} created successfully`);
}

main();
```

## Boundary Checker Pattern

```typescript
// check-boundaries.ts
#!/usr/bin/env npx ts-node

import * as fs from 'fs';
import * as path from 'path';
import * as ts from 'typescript';

interface BoundaryRule {
  from: string;
  canImport: string[];
  cannotImport: string[];
}

const RULES: BoundaryRule[] = [
  {
    from: 'src/lib/utils',
    canImport: ['src/lib/types'],
    cannotImport: ['src/lib/components', 'src/lib/stores', 'src/lib/services', 'src/routes'],
  },
  {
    from: 'src/lib/types',
    canImport: [],
    cannotImport: ['src/lib/components', 'src/lib/stores', 'src/lib/services', 'src/lib/utils', 'src/routes'],
  },
  {
    from: 'src/lib/services',
    canImport: ['src/lib/utils', 'src/lib/types'],
    cannotImport: ['src/lib/components', 'src/lib/stores', 'src/routes'],
  },
  {
    from: 'src/lib/stores',
    canImport: ['src/lib/services', 'src/lib/utils', 'src/lib/types'],
    cannotImport: ['src/lib/components', 'src/routes'],
  },
  {
    from: 'src/lib/components',
    canImport: ['src/lib/stores', 'src/lib/services', 'src/lib/utils', 'src/lib/types'],
    cannotImport: ['src/routes'],
  },
];

function checkFile(filePath: string): string[] {
  const violations: string[] = [];
  const content = fs.readFileSync(filePath, 'utf-8');
  const sourceFile = ts.createSourceFile(filePath, content, ts.ScriptTarget.Latest, true);

  // Find the rule that applies to this file
  const rule = RULES.find(r => filePath.includes(r.from));
  if (!rule) return violations;

  // Walk the AST to find imports
  ts.forEachChild(sourceFile, function visit(node) {
    if (ts.isImportDeclaration(node)) {
      const moduleSpecifier = node.moduleSpecifier;
      if (ts.isStringLiteral(moduleSpecifier)) {
        const importPath = moduleSpecifier.text;

        // Check against cannotImport rules
        for (const forbidden of rule.cannotImport) {
          if (importPath.includes(forbidden.replace('src/', '$'))) {
            violations.push(
              `${filePath}: Cannot import from ${forbidden} (imported: ${importPath})`
            );
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  });

  return violations;
}

function main() {
  const srcDir = path.join(__dirname, '../src');
  const violations: string[] = [];

  function walkDir(dir: string) {
    const files = fs.readdirSync(dir);
    for (const file of files) {
      const fullPath = path.join(dir, file);
      const stat = fs.statSync(fullPath);

      if (stat.isDirectory()) {
        walkDir(fullPath);
      } else if (file.endsWith('.ts') || file.endsWith('.svelte')) {
        violations.push(...checkFile(fullPath));
      }
    }
  }

  walkDir(srcDir);

  if (violations.length > 0) {
    console.error('Architecture boundary violations found:\n');
    for (const v of violations) {
      console.error(`  ✗ ${v}`);
    }
    console.error(`\n${violations.length} violation(s) found`);
    process.exit(1);
  }

  console.log('✓ No architecture boundary violations found');
}

main();
```

## Adding New Scripts

1. Create the script in `scripts/`
2. Add executable permission: `chmod +x scripts/my-script.ts`
3. Add npm script in `package.json`:
   ```json
   {
     "scripts": {
       "my-script": "ts-node scripts/my-script.ts"
     }
   }
   ```
4. Document in this README

## Architectural Rules

1. Scripts must be **idempotent** where possible
2. Scripts must **validate input** and provide helpful errors
3. Scripts must **not modify** application source behavior
4. Use **TypeScript** for complex scripts (not shell)
5. Keep scripts **focused** - one task per script
