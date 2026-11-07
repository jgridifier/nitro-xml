// SPDX-License-Identifier: MIT
/** Explicit "not recorded by the source run" marker for protocol-key fields. It never equals anything, itself included. */
export const UNKNOWN = Object.freeze({ unknown: true });
export const isUnknown = (v) => !!v && typeof v === 'object' && v.unknown === true;
