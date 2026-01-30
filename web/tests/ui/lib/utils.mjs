export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function waitForHttpOk(url, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const resp = await fetch(url, { method: 'GET' });
      if (resp.ok) return;
    } catch {
      // ignore and retry
    }
    await sleep(250);
  }
  throw new Error(`web dev server did not become ready at ${url}`);
}

export async function waitForDataAttribute(locator, attr, expected, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const value = await locator.getAttribute(attr);
    if (value === expected) return;
    await sleep(250);
  }
  const value = await locator.getAttribute(attr);
  throw new Error(`expected ${attr}=${expected}, got ${value ?? 'null'}`);
}

export async function waitForLocatorCount(locator, expected, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const count = await locator.count();
    if (count === expected) return;
    await sleep(250);
  }
  const count = await locator.count();
  throw new Error(`expected ${expected} matches, got ${count}`);
}

