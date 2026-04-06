interface BuildInfo {
  sha: string;
  date: string;
  ref: string;
}

/** Fetch build metadata and display in UI + console. */
export async function initVersion(): Promise<void> {
  try {
    const resp = await fetch('./version.json');
    if (!resp.ok) return;
    const info: BuildInfo = await resp.json();
    const short = `${info.sha} (${info.date.slice(0, 10)})`;
    console.info(`RustCAM ${info.ref} ${short}`);
    const el = document.createElement('span');
    el.className = 'text-text-dim text-[11px] font-mono ml-2';
    el.textContent = short;
    el.title = `Branch: ${info.ref}\nSHA: ${info.sha}\nBuilt: ${info.date}`;
    document.querySelector('header')?.appendChild(el);
  } catch {
    // version.json missing in local dev — silently ignore
  }
}
