// @ts-check

/**
 * groupBySync — apply groupBy + sort-direction state to the group-by menu UI.
 *
 * Pure DOM helper with no module imports so it can be safely imported by any
 * view without creating circular dependencies.
 *
 * Call after `syncGroupByMenu()` has built the option list, e.g. when
 * restoring saved preferences on section entry.
 *
 * Usage:
 *   import { applyGroupByMenuState } from '../core/groupBySync.js';
 *   applyGroupByMenuState('type', true);
 */

/**
 * Reflect `groupBy` and `reversed` in the group-by menu DOM:
 *   - marks the matching `.group-by-option` as active
 *   - updates the group-by button label and active class
 *   - toggles the sort-direction button active class
 *
 * No-op when the menu elements are not in the DOM (e.g. before initApp).
 *
 * @param {string}  groupBy   Active group-by key, e.g. `''` for the "Name"
 *                            / flat-sort option a section opts into.
 * @param {boolean} reversed  Whether sort direction is reversed.
 */
function applyGroupByMenuState(groupBy, reversed) {
    // Mark the matching option as active; clear all others.
    for (const b of document.querySelectorAll('.group-by-option')) {
        const btn = /** @type {HTMLElement} */ (b);
        btn.classList.toggle('active', (btn.dataset.groupBy ?? '') === groupBy);
    }

    // Group-by button always reflects the active option (icon + label).
    const groupByBtn = document.getElementById('group-by-btn');
    groupByBtn?.classList.add('active');

    const lbl = groupByBtn?.querySelector('.group-by-label');
    if (lbl) {
        const activeOpt = /** @type {HTMLElement|null} */ (document.querySelector(`.group-by-option[data-group-by="${CSS.escape(groupBy)}"]`));
        lbl.innerHTML = activeOpt?.innerHTML ?? '';
    }

    // Sort-direction button: active = reversed.
    document.getElementById('sort-dir-btn')?.classList.toggle('active', reversed);
}

export { applyGroupByMenuState };
