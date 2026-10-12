# Mobile text surface smoke (#60)

Automated coverage uses Playwright Chromium with **iPhone 13** and **Pixel 5** device
descriptors (keyboard + touch emulation). Real-device checks below are optional smoke
before release.

## iOS Safari

- [ ] Focus cell editor / formula bar; virtual keyboard appears.
- [ ] Type letters; predictive bar suggestions insert on tap.
- [ ] Autocorrect replaces a mistyped word on space.
- [ ] Backspace deletes graphemes; long-press backspace deletes words.
- [ ] Drag selection handles; native copy/cut/paste menu appears.
- [ ] `enterkeyhint` matches field (Done vs Enter).

## Android Chrome

- [ ] Same typing, deletion, and suggestion/autocorrect behaviour.
- [ ] Gboard predictive strip commits insert text.
- [ ] Touch handles adjust selection; context menu actions work.

## Negative control

- [ ] Read-only sheet: no handles, no autocorrect rewrite, keyboard does not edit.
