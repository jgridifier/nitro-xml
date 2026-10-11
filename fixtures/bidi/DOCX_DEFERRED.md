<!-- SPDX-License-Identifier: MIT -->
# Deferred docx bidi fixture

RTL paragraphs and mixed-direction runs for `.docx` will be added when
Print Layout paint hosts land (#23). The case is registered in
`tests/bidi/registry.mjs` as `ready: false` so the Stage-3 bidi gate can
arm now for xlsx/pptx without blocking on Word.
