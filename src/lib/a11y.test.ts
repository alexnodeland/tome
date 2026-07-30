import { describe, it, expect, beforeEach } from 'vitest';
import { trapFocus } from './a11y';

/** A dialog with three controls and a button outside it. */
function scene() {
  document.body.innerHTML = `
    <button id="outside">outside</button>
    <div id="dialog" tabindex="-1">
      <button id="first">first</button>
      <input id="middle" />
      <button id="last">last</button>
      <button id="disabled" disabled>disabled</button>
    </div>
  `;
  const dialog = document.getElementById('dialog') as HTMLElement;
  const action = trapFocus(dialog);
  const el = (id: string) => document.getElementById(id) as HTMLElement;
  const tab = (shiftKey = false) => {
    const event = new KeyboardEvent('keydown', {
      key: 'Tab',
      shiftKey,
      bubbles: true,
      cancelable: true,
    });
    (document.activeElement ?? dialog).dispatchEvent(event);
    return event;
  };
  return { dialog, action, el, tab };
}

describe('trapFocus', () => {
  beforeEach(() => {
    // jsdom reports `offsetParent` as null for everything, which would make
    // the tabbable filter reject every element. Give the layout box the one
    // property the filter reads.
    Object.defineProperty(HTMLElement.prototype, 'offsetParent', {
      configurable: true,
      get() {
        return this.hasAttribute('hidden') ? null : document.body;
      },
    });
  });

  it('wraps forward from the last enabled control to the first', () => {
    // `#disabled` comes after `#last` in the DOM, so this also proves the
    // tabbable filter excludes it: if it did not, `#last` would not be the
    // edge and nothing would be intercepted.
    // `aria-modal="true"` tells assistive technology the rest of the page is
    // inert and does nothing whatever to the keyboard. Without this, Tab here
    // moves into the shell behind the overlay: the focus ring vanishes and the
    // next Return activates something the user cannot see.
    const { el, tab } = scene();
    el('last').focus();
    const event = tab();
    expect(event.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(el('first'));
  });

  it('wraps backward from the first control to the last', () => {
    const { el, tab } = scene();
    el('first').focus();
    const event = tab(true);
    expect(event.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(el('last'));
  });

  it('leaves the middle of the tab order to the browser', () => {
    // Only the two edges are intercepted; reimplementing the order in between
    // would handle `tabindex`, disabled controls and shadow roots worse.
    const { el, tab } = scene();
    el('first').focus();
    const event = tab();
    expect(event.defaultPrevented).toBe(false);
  });

  it('pulls focus in when it is on the dialog container itself', () => {
    // Both modals put `tabindex="-1"` on the dialog and focus it when they
    // open, so this is the state every Tab starts from.
    const { dialog, el, tab } = scene();
    dialog.focus();
    tab();
    expect(document.activeElement).toBe(el('first'));
  });

  it('stops listening once destroyed', () => {
    const { el, tab, action } = scene();
    action.destroy();
    el('last').focus();
    const event = tab();
    expect(event.defaultPrevented).toBe(false);
    expect(document.activeElement).toBe(el('last'));
  });
});
