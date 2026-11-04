// Downloads a gateway file by its SHA-256 and refuses bytes that do not hash to it.

const hex = (bytes: ArrayBuffer): string =>
  Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, "0")).join("");

export class ContentHashMismatch extends Error {
  constructor(expected: string, actual: string) {
    super(`content ${expected} downloaded with SHA-256 ${actual}`);
  }
}

/** `GET /content/{sha256}`; the response is immutable, so the browser cache may serve it. */
export async function fetchVerified(sha256: string, onProgress?: (loaded: number) => void): Promise<Uint8Array> {
  const response = await fetch(`/content/${sha256}`);
  if (!response.ok || response.body === null) {
    throw new Error(`content ${sha256}: HTTP ${response.status}`);
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    loaded += value.length;
    onProgress?.(loaded);
  }
  const bytes = new Uint8Array(loaded);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  const actual = hex(await crypto.subtle.digest("SHA-256", bytes));
  if (actual !== sha256) throw new ContentHashMismatch(sha256, actual);
  return bytes;
}
