/** Typed DOM element accessors. */

export function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Element #${id} not found`);
  return el;
}

export function $input(id: string): HTMLInputElement {
  return $(id) as HTMLInputElement;
}

export function $select(id: string): HTMLSelectElement {
  return $(id) as HTMLSelectElement;
}

export function $canvas(id: string): HTMLCanvasElement {
  return $(id) as HTMLCanvasElement;
}

export function $textarea(id: string): HTMLTextAreaElement {
  return $(id) as HTMLTextAreaElement;
}

export function $btn(id: string): HTMLButtonElement {
  return $(id) as HTMLButtonElement;
}
