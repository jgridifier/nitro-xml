// SPDX-License-Identifier: MIT
import React, { useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { NitroViewer } from '@nitro-xml/react';

function App() {
  const ref = useRef(null);
  const [status, setStatus] = useState('loading');
  const file = new URLSearchParams(location.search).get('file');
  return <><output id="status">{status}</output><NitroViewer ref={ref}
    file={`/fixtures/${file}`} fileName={file} editable style={{ height: 600 }}
    onReady={() => { window.consumer = ref.current; setStatus('ready'); }}
    onError={error => setStatus(`error: ${error.message}`)} /></>;
}
createRoot(document.getElementById('root')).render(<App />);
