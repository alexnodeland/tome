# Design System

This document defines the visual language, components, and patterns for Tome's user interface.

---

## Design Principles

1. **Content-first** - Documentation is the star, chrome is minimal
2. **Typography matters** - Reading should feel like a well-set book
3. **Native feel** - Respect macOS conventions and behaviors
4. **Accessible** - Usable by keyboard, readable by screen readers
5. **Consistent** - Same patterns throughout the application

---

## Color System

### Semantic Colors

```css
:root {
  /* Brand */
  --color-accent: #5856D6;
  --color-accent-hover: #4845B5;
  --color-accent-active: #3D3A9E;

  /* Backgrounds */
  --color-bg-primary: #FAFAFA;
  --color-bg-secondary: #FFFFFF;
  --color-bg-tertiary: #F5F5F7;

  /* Text
     Contrast against --color-bg-primary (#FAFAFA), WCAG 2.1:
       primary   #1D1D1F  16.8:1  AAA
       secondary #6E6E73   4.9:1  AA  (body text)
       tertiary  #57575B   6.4:1  AA  -- was #8E8E93 at 3.1:1, which FAILED
                                         the 4.5:1 requirement this project
                                         sets for itself in the NFR document. */
  --color-text-primary: #1D1D1F;
  --color-text-secondary: #6E6E73;
  --color-text-tertiary: #57575B;
  --color-text-inverse: #FFFFFF;

  /* Borders */
  --color-border: #E5E5EA;
  --color-border-strong: #D1D1D6;

  /* Status */
  --color-success: #34C759;
  --color-warning: #FF9500;
  --color-error: #FF3B30;
  --color-info: #5856D6;

  /* Code — #F5F5F7 against the page background #FAFAFA is a 1.02:1 difference,
     effectively invisible. Code blocks need a discernible edge, so the surface
     is slightly deeper and a border carries the boundary. */
  --color-code-bg: #F1F1F4;
  --color-code-text: #1D1D1F;
  --color-code-border: #E5E5EA;

  /* Highlight colours: chosen so black body text remains >= 4.5:1 on top of
     them. A saturated yellow wash fails this and makes highlighted text the
     least readable text on the page. */
  --color-highlight-yellow: #FFF3B0;
  --color-highlight-green:  #D6F5D6;
  --color-highlight-blue:   #D6E9FF;
  --color-highlight-pink:   #FFE0EC;
}

/* Dark mode */
@media (prefers-color-scheme: dark) {
  :root {
    /* Brand */
    --color-accent: #5E5CE6;
    --color-accent-hover: #7674E8;
    --color-accent-active: #8E8CEA;

    /* Backgrounds */
    --color-bg-primary: #1C1C1E;
    --color-bg-secondary: #2C2C2E;
    --color-bg-tertiary: #38383A;

    /* Text — contrast against --color-bg-primary (#1C1C1E):
         primary   #F5F5F7  15.8:1  AAA
         secondary #98989D   6.3:1  AA
         tertiary  #8A8A8F   5.1:1  AA  -- was #6E6E73 at 3.4:1, which FAILED */
    --color-text-primary: #F5F5F7;
    --color-text-secondary: #98989D;
    --color-text-tertiary: #8A8A8F;

    /* Borders */
    --color-border: #38383A;
    --color-border-strong: #48484A;

    /* Code — must differ from --color-bg-secondary (#2C2C2E), or code blocks
       are invisible on every surface that uses it (panels, cards, modals).
       The original set them to the same value. */
    --color-code-bg: #222224;
    --color-code-text: #F5F5F7;
    --color-code-border: #3A3A3C;

    /* Status colors need dark-mode variants too; the original block omitted
       them, so light-mode values were used on dark backgrounds. */
    --color-success: #30D158;
    --color-warning: #FF9F0A;
    --color-error:   #FF453A;
    --color-info:    #5E5CE6;
  }
}

/* Manual theme override.
   P5-007 offers light / dark / system, but the original stylesheet had only a
   `prefers-color-scheme` media query -- which cannot honour an explicit user
   choice. The app sets data-theme on <html>; these blocks must be able to win
   in both directions. */
:root[data-theme="dark"]  { /* ...dark token values... */ }
:root[data-theme="light"] { /* ...light token values... */ }
```

### Color Palette Reference

| Name | Light | Dark | Usage |
|------|-------|------|-------|
| Accent | `#5856D6` | `#5E5CE6` | Button and indicator **fills**, active states. **Not link text and not the focus ring** -- see the contrast findings below |
| Link | `#5856D6` | `#9D9BF5` | Link text, in the chrome and in the reader |
| Focus | `#5856D6` | `#9D9BF5` | The focus ring, on every surface it can appear over |
| Background | `#FAFAFA` | `#1C1C1E` | App background |
| Surface | `#FFFFFF` | `#2C2C2E` | Cards, panels |
| Text Primary | `#1D1D1F` | `#F5F5F7` | Body text |
| Text Secondary | `#6E6E73` | `#98989D` | Labels, captions |
| Border | `#E5E5EA` | `#38383A` | Dividers, outlines |

---

## Typography

### Font Stack

```css
:root {
  /* Body text - serif for readability */
  --font-body: 'New York', 'Georgia', 'Times New Roman', serif;

  /* Headings - system sans */
  --font-heading: -apple-system, 'SF Pro Display', BlinkMacSystemFont, sans-serif;

  /* Code - monospace */
  --font-mono: 'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace;

  /* UI elements - system sans */
  --font-ui: -apple-system, 'SF Pro Text', BlinkMacSystemFont, sans-serif;
}
```

### Type Scale

> **The original scale did not produce the sizes it documented.** `--text-base: 17px` was declared
> but never applied to anything, so `rem` resolved against the browser default of 16px: `1rem` was
> 16px (not 17), `--text-xs` was 10.24px (not 10.9), `--text-sm` was 12.8px (not 13.6). Every
> commented value was wrong, and the reader would have rendered a point smaller than the PRD
> specifies. Setting the root font size is what makes `rem` mean what the comments claim — and it
> also makes the user's font-size preference work, since changing one value rescales the system.

```css
/* This line is what makes every rem below correct. */
html { font-size: 17px; }

:root {
  /* User preference multiplies this; everything scales from it. */
  --text-base: 17px;

  /* Scale: 1.25 ratio, relative to the 17px root */
  --text-xs:  0.64rem;  /* 10.9px */
  --text-sm:  0.8rem;   /* 13.6px */
  --text-md:  1rem;     /* 17px   */
  --text-lg:  1.25rem;  /* 21.3px */
  --text-xl:  1.563rem; /* 26.6px */
  --text-2xl: 1.953rem; /* 33.2px */
  --text-3xl: 2.441rem; /* 41.5px */

  /* Code: the PRD specifies 15px, which the scale does not contain.
     Do not substitute --text-sm (13.6px) -- that was the drift between
     this document and the PRD. */
  --text-code: 0.882rem;  /* 15px */

  /* Line heights */
  --leading-tight:   1.25;
  --leading-normal:  1.5;
  --leading-relaxed: 1.6;
  --leading-loose:   1.75;
}

/* Reader font size preference (P5-007) rescales the whole system. */
:root[data-text-size="small"]  { font-size: 15px; }
:root[data-text-size="large"]  { font-size: 19px; }
:root[data-text-size="xlarge"] { font-size: 21px; }
```

**Minimum size floor.** Nothing renders below 11px. `--text-xs` at 10.9px is already at the edge of
legibility, and it is used for `.caption`, which is also the lowest-contrast token — the two
choices compound. Prefer `--text-sm` for captions.

### Typography Classes

```css
/* Headings */
.heading-1 {
  font-family: var(--font-heading);
  font-size: var(--text-3xl);
  font-weight: 600;
  line-height: var(--leading-tight);
  letter-spacing: -0.02em;
}

.heading-2 {
  font-family: var(--font-heading);
  font-size: var(--text-2xl);
  font-weight: 600;
  line-height: var(--leading-tight);
  letter-spacing: -0.01em;
}

.heading-3 {
  font-family: var(--font-heading);
  font-size: var(--text-xl);
  font-weight: 600;
  line-height: var(--leading-normal);
}

/* Body */
.body {
  font-family: var(--font-body);
  font-size: var(--text-md);
  line-height: var(--leading-relaxed);
}

.body-small {
  font-family: var(--font-body);
  font-size: var(--text-sm);
  line-height: var(--leading-normal);
}

/* Code */
.code {
  font-family: var(--font-mono);
  font-size: 0.9em;  /* Slightly smaller than surrounding text */
}

/* UI */
.label {
  font-family: var(--font-ui);
  font-size: var(--text-sm);
  font-weight: 500;
}

.caption {
  font-family: var(--font-ui);
  font-size: var(--text-xs);
  color: var(--color-text-secondary);
}
```

### Reader Typography

```css
.reader-content {
  font-family: var(--font-body);
  font-size: var(--text-md);
  line-height: var(--leading-relaxed);
  max-width: 70ch;  /* Optimal measure */
  margin: 0 auto;
  padding: 2rem;
}

.reader-content p {
  margin-bottom: 1em;
}

.reader-content h1,
.reader-content h2,
.reader-content h3 {
  font-family: var(--font-heading);
  margin-top: 2em;
  margin-bottom: 0.5em;
}

.reader-content code {
  font-family: var(--font-mono);
  font-size: 0.9em;
  background: var(--color-code-bg);
  padding: 0.1em 0.3em;
  border-radius: 3px;
}

.reader-content pre {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  background: var(--color-code-bg);
  padding: 1rem;
  border-radius: 6px;
  overflow-x: auto;
}
```

---

## Spacing

### Spacing Scale

```css
:root {
  --space-0: 0;
  --space-1: 0.25rem;  /* 4px */
  --space-2: 0.5rem;   /* 8px */
  --space-3: 0.75rem;  /* 12px */
  --space-4: 1rem;     /* 16px */
  --space-5: 1.5rem;   /* 24px */
  --space-6: 2rem;     /* 32px */
  --space-8: 3rem;     /* 48px */
  --space-10: 4rem;    /* 64px */
  --space-12: 6rem;    /* 96px */
}
```

### Layout Spacing

```css
:root {
  /* Sidebar widths */
  --sidebar-width-left: 240px;
  --sidebar-width-right: 200px;
  --sidebar-min-width: 180px;
  --sidebar-max-width: 400px;

  /* Content areas */
  --content-padding: var(--space-5);
  --content-max-width: 900px;

  /* Component spacing */
  --gap-tight: var(--space-2);
  --gap-normal: var(--space-4);
  --gap-loose: var(--space-6);
}
```

---

## Components

### Buttons

```svelte
<!-- Button.svelte -->
<script>
  export let variant: 'primary' | 'secondary' | 'ghost' = 'secondary';
  export let size: 'sm' | 'md' | 'lg' = 'md';
  export let disabled = false;
</script>

<button
  class="button button--{variant} button--{size}"
  {disabled}
  on:click
>
  <slot />
</button>

<style>
  .button {
    font-family: var(--font-ui);
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .button--primary {
    background: var(--color-accent);
    color: var(--color-text-inverse);
    border: none;
  }

  .button--primary:hover {
    background: var(--color-accent-hover);
  }

  .button--secondary {
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    border: 1px solid var(--color-border);
  }

  .button--secondary:hover {
    background: var(--color-bg-tertiary);
  }

  .button--ghost {
    background: transparent;
    color: var(--color-text-primary);
    border: none;
  }

  .button--ghost:hover {
    background: var(--color-bg-tertiary);
  }

  .button--sm {
    font-size: var(--text-sm);
    padding: var(--space-1) var(--space-3);
  }

  .button--md {
    font-size: var(--text-md);
    padding: var(--space-2) var(--space-4);
  }

  .button--lg {
    font-size: var(--text-lg);
    padding: var(--space-3) var(--space-5);
  }

  .button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

### Input Fields

```svelte
<!-- Input.svelte -->
<script>
  export let type = 'text';
  export let placeholder = '';
  export let value = '';
  export let error = '';
</script>

<div class="input-wrapper" class:has-error={error}>
  <input
    {type}
    {placeholder}
    bind:value
    class="input"
    on:input
    on:focus
    on:blur
  />
  {#if error}
    <span class="input-error">{error}</span>
  {/if}
</div>

<style>
  .input {
    font-family: var(--font-ui);
    font-size: var(--text-md);
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    width: 100%;
    transition: border-color 0.15s ease;
  }

  .input:focus-visible {
    outline: none;
    border-color: var(--color-accent);
    /* Derived from the accent token rather than a hardcoded rgba() of the
       light-mode accent -- the original literal stayed light-mode indigo in
       dark mode. */
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 25%, transparent);
  }

  .has-error .input {
    border-color: var(--color-error);
  }

  .input-error {
    font-size: var(--text-sm);
    color: var(--color-error);
    margin-top: var(--space-1);
  }
</style>
```

### Search Box

```svelte
<!-- SearchBox.svelte -->
<script>
  export let placeholder = 'Search...';
  export let value = '';
  export let loading = false;
</script>

<div class="search-box">
  <svg class="search-icon" viewBox="0 0 20 20">
    <!-- magnifying glass icon -->
  </svg>
  <input
    type="search"
    {placeholder}
    bind:value
    class="search-input"
  />
  {#if loading}
    <div class="search-spinner" />
  {/if}
  {#if value}
    <button class="search-clear" on:click={() => value = ''}>
      <svg viewBox="0 0 20 20"><!-- x icon --></svg>
    </button>
  {/if}
</div>

<style>
  .search-box {
    display: flex;
    align-items: center;
    background: var(--color-bg-tertiary);
    border-radius: 8px;
    padding: var(--space-2) var(--space-3);
    gap: var(--space-2);
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-family: var(--font-ui);
    font-size: var(--text-md);
    color: var(--color-text-primary);
  }

  .search-input:focus {
    outline: none;
  }

  .search-icon {
    width: 16px;
    height: 16px;
    color: var(--color-text-secondary);
  }
</style>
```

### List Items

```svelte
<!-- ListItem.svelte -->
<script>
  export let selected = false;
  export let icon = '';
  export let title = '';
  export let subtitle = '';
</script>

<!-- role="button" with only a click handler is keyboard-inaccessible: it is
     focusable and announced as a button, but Enter and Space do nothing. This
     contradicts design principle #4 and the NFR keyboard requirement. -->
<div
  class="list-item"
  class:selected
  role="button"
  tabindex="0"
  aria-pressed={selected}
  on:click
  on:keydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();          // Space must not scroll the page
      e.currentTarget.click();
    }
  }}
>
  {#if icon}
    <span class="list-item-icon">{icon}</span>
  {/if}
  <div class="list-item-content">
    <span class="list-item-title">{title}</span>
    {#if subtitle}
      <span class="list-item-subtitle">{subtitle}</span>
    {/if}
  </div>
</div>

<style>
  .list-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.1s ease;
  }

  .list-item:hover {
    background: var(--color-bg-tertiary);
  }

  .list-item.selected {
    background: var(--color-accent);
    color: var(--color-text-inverse);
  }

  .list-item-icon {
    font-size: var(--text-lg);
  }

  .list-item-title {
    font-family: var(--font-ui);
    font-size: var(--text-md);
    font-weight: 500;
  }

  .list-item-subtitle {
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  .list-item.selected .list-item-subtitle {
    color: rgba(255, 255, 255, 0.7);
  }
</style>
```

---

## Layout Patterns

### Three-Panel Layout

```svelte
<!-- Layout.svelte -->
<script>
  export let leftOpen = true;
  export let rightOpen = true;
</script>

<div class="app-layout">
  {#if leftOpen}
    <aside class="sidebar sidebar--left">
      <slot name="left" />
    </aside>
  {/if}

  <main class="main-content">
    <slot name="main" />
  </main>

  {#if rightOpen}
    <aside class="sidebar sidebar--right">
      <slot name="right" />
    </aside>
  {/if}
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    background: var(--color-bg-primary);
  }

  .sidebar {
    background: var(--color-bg-secondary);
    border-color: var(--color-border);
    overflow-y: auto;
  }

  .sidebar--left {
    width: var(--sidebar-width-left);
    border-right: 1px solid var(--color-border);
  }

  .sidebar--right {
    width: var(--sidebar-width-right);
    border-left: 1px solid var(--color-border);
  }

  .main-content {
    flex: 1;
    overflow-y: auto;
  }
</style>
```

### Modal / Dialog

```svelte
<!-- Modal.svelte -->
<script>
  export let open = false;
  export let title = '';
</script>

<!-- The original modal had no focus trap, no Escape handler, no aria-modal,
     and did not restore focus on close. A keyboard or screen-reader user could
     tab out of the dialog into the page behind it and lose their place -- and
     the close button's label was the character "×", which VoiceOver reads as
     "multiplication sign". -->
<svelte:window on:keydown={(e) => { if (open && e.key === 'Escape') close(); }} />

{#if open}
  <div class="modal-backdrop" on:click|self={close}>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
      bind:this={dialogEl}
      use:trapFocus
    >
      <header class="modal-header">
        <h2 id="modal-title">{title}</h2>
        <button class="modal-close" on:click={close} aria-label="Close dialog">
          <Icon name="x" />
        </button>
      </header>
      <div class="modal-body">
        <slot />
      </div>
      <footer class="modal-footer">
        <slot name="footer" />
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: var(--color-bg-secondary);
    border-radius: 12px;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
    max-width: 500px;
    width: 90%;
    max-height: 90vh;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  .modal-body {
    padding: var(--space-4);
    overflow-y: auto;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    padding: var(--space-4);
    border-top: 1px solid var(--color-border);
  }
</style>
```

---

## Icons

### Icon System

Use SF Symbols or a consistent icon set:

```svelte
<!-- Icon.svelte -->
<script>
  export let name: string;
  export let size: number = 16;
</script>

<!-- `aria-hidden` unconditionally means an icon-only button has NO accessible
     name -- screen readers announce "button" and nothing else. Several
     icon-only buttons exist (close, clear search, bookmark, sync). Icons are
     decorative by default and labellable when they carry meaning. -->
<script>
  export let name: string;
  export let size: number = 16;
  /** Set when the icon IS the label (icon-only buttons). */
  export let label: string | undefined = undefined;
</script>

<svg
  class="icon"
  width={size}
  height={size}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
  focusable="false"
>
  <use href="/icons.svg#{name}" />
</svg>

<style>
  .icon {
    display: inline-block;
    vertical-align: middle;
    fill: currentColor;
  }
</style>
```

### Icon Names

| Name | Usage |
|------|-------|
| `search` | Search actions |
| `bookmark` | Bookmark indicator |
| `bookmark-fill` | Active bookmark |
| `arrow-left` | Back navigation |
| `arrow-right` | Forward navigation |
| `sync` | Sync indicator |
| `check` | Success state |
| `x` | Close, error |
| `chevron-right` | Expandable items |
| `folder` | Collections |
| `doc` | Documentation source |

---

## Motion

### Transitions

```css
:root {
  --duration-fast: 100ms;
  --duration-normal: 200ms;
  --duration-slow: 300ms;

  --ease-in: cubic-bezier(0.4, 0, 1, 1);
  --ease-out: cubic-bezier(0, 0, 0.2, 1);
  --ease-in-out: cubic-bezier(0.4, 0, 0.2, 1);
}

/* Respect user preference */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    /* The original rule missed this, so TOC clicks and "scroll to match" still
       animated -- and smooth scrolling is the motion most likely to cause
       discomfort for the people this preference exists to protect. */
    scroll-behavior: auto !important;
  }
}
```

### Common Animations

```css
/* Fade in */
@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* Slide in from right (for sidebars) */
@keyframes slide-in-right {
  from { transform: translateX(100%); }
  to { transform: translateX(0); }
}

/* Spin (for loading) */
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
```

---

## Accessibility

### Focus States

```css
/* Visible focus for keyboard users */
:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

/* Remove default outline for mouse users */
:focus:not(:focus-visible) {
  outline: none;
}

/* On the accent-filled selected row, an accent outline is invisible.
   Use the inverse token so focus is never lost against its own background. */
.list-item.selected:focus-visible {
  outline-color: var(--color-text-inverse);
}
```

### Accessibility checklist for every component

Applied at review time. Each item exists because the original components missed it:

- [ ] Interactive non-`<button>` elements handle **Enter and Space**, not only click
- [ ] Icon-only controls have an accessible name
- [ ] Dialogs: `aria-modal`, focus trap, Escape, focus restored to the trigger on close
- [ ] Focus is visible against **every** background the element can sit on
- [ ] Colour is never the only carrier of meaning (sync state needs an icon shape, not just red)
- [ ] Text and its background meet 4.5:1; **boundaries that carry meaning** -- focus rings, input
      outlines, control edges -- meet 3:1; **verified against the tokens, not by eye**. A hairline
      between two panels that are already distinguished by their fills is decoration, not a
      boundary WCAG 1.4.11 governs; the earlier "UI borders meet 3:1" was too broad to be true of
      any palette this project would ship. `scripts/check-contrast.mjs` enumerates which pairs are
      asserted and, just as importantly, which are not and why
- [ ] Component is reachable and operable with the pointer unplugged

### Automated contrast checking

**Built in S1-12. `scripts/check-contrast.mjs` runs in `scripts/check.sh` and in CI, and fails the
build.** It parses `public/tokens.css` -- the file these token blocks now *are* -- and asserts
three things:

1. Every foreground/background pair that can legitimately combine meets its ratio, in both themes.
2. Light and dark define the same set of colour tokens. A token with no dark variant silently uses
   its light value on a dark background, which is what the original block did with the status
   colours.
3. The `@media (prefers-color-scheme: dark)` block and the `:root[data-theme="dark"]` override are
   byte-identical. They must be duplicated (a media query cannot honour an explicit choice), and
   duplication nothing checks is duplication that drifts.

Its first run found three real defects in the palette this document specified, none of which are
visible by eye:

| Defect | Measured | Fix |
|---|---|---|
| Dark `--color-accent` used as link text | 3.36:1 on `--color-bg-primary` | new `--color-link`, `#9D9BF5` in dark |
| Dark `--color-accent` used as a focus ring | 2.70:1 on `--color-bg-secondary` | new `--color-focus`, same value |
| Status colours used as *text* (admonition titles, error messages) | 1.96:1 for `--color-warning` on a panel | new `--color-success-text` / `--color-warning-text` / `--color-error-text` |

`--color-accent` keeps its documented value: it is correct for a *fill*, and the three defects were
all uses of it as something other than a fill. `--color-text-inverse` is `#FFFFFF` in **both**
themes for the same reason -- white on the accent fill measures 5.6:1 light and 5.1:1 dark, while
the "obvious" dark-mode value of dark-on-accent measures 3.3:1.

The pairs the script deliberately does **not** assert are listed in it with reasons, which matters
as much as the ones it does: panel dividers (~1.2:1, decoration under 1.4.11) and status *fills*
(2.2:1, legitimate only because this document forbids colour as the sole carrier of meaning).

### Screen Reader Utilities

```css
/* Visually hidden but accessible */
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
```

---

## Responsive Behavior

### Keyboard shortcuts

**Canonical list: [PRD Appendix C](../PRD.md#appendix-c-keyboard-shortcut-reference).** Not
restated here — four partially-contradictory copies previously existed across the plan set, two of
which shadowed macOS system shortcuts (`Cmd+H` Hide, `Cmd+P` Print).

Component-level rules that follow from it:

- Single-letter reading keys (`J`, `K`, `G`, `[`, `]`) bind on the reader surface only, and every
  handler returns early if the focused element is an input, textarea, or `contenteditable`.
  Otherwise typing "j" in the source filter box scrolls the document.
- Every shortcut also appears in the menu bar next to its command, which is how macOS users
  discover shortcuts and how VoiceOver announces them.

### Breakpoints

```css
:root {
  --breakpoint-sm: 640px;
  --breakpoint-md: 768px;
  --breakpoint-lg: 1024px;
  --breakpoint-xl: 1280px;
}

/* Example: collapse sidebars on narrow windows */
@media (max-width: 1024px) {
  .sidebar--right {
    display: none;
  }
}

@media (max-width: 768px) {
  .sidebar--left {
    position: absolute;
    z-index: 50;
  }
}
```

### Minimum Window Size

- **Width:** 800px
- **Height:** 600px

(Enforced in window configuration)
