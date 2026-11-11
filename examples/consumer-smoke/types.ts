// SPDX-License-Identifier: MIT
// Catches: published declaration entry points that cannot resolve under Node ESM.
import type { Handle } from 'nitro-xml';
import type { NitroViewerHandle, NitroViewerProps } from '@nitro-xml/react';
export type Consumer = { engine: Handle; viewer: NitroViewerHandle; props: NitroViewerProps };
