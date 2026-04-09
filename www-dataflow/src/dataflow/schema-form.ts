/**
 * Vanilla TypeScript JSON Schema form renderer.
 *
 * Takes a JSON Schema object (as produced by Rust schemars) and renders an
 * HTML form using plain DOM APIs.  No React, no npm dependencies.
 */

/**
 * Render an HTML form from a JSON Schema.
 *
 * @param container - DOM element to render into (cleared first)
 * @param schema - JSON Schema object (from Rust schemars)
 * @param data - Current values to populate form with
 * @param onChange - Called with updated data when any field changes
 */
export function renderSchemaForm(
  container: HTMLElement,
  schema: Record<string, unknown>,
  data: Record<string, unknown>,
  onChange: (data: Record<string, unknown>) => void,
): void {
  container.textContent = '';

  const properties = schema.properties as
    | Record<string, Record<string, unknown>>
    | undefined;
  if (!properties) return;

  const currentData = { ...data };

  for (const [key, propSchema] of Object.entries(properties)) {
    const value = currentData[key];
    const row = document.createElement('div');
    row.className = 'mb-2';

    // Label
    const label = document.createElement('label');
    label.className = 'block text-text-dim text-[11px] mb-0.5';
    label.textContent = (propSchema.title as string | undefined) ?? key;
    row.appendChild(label);

    // Input based on type
    const type = propSchema.type as string | undefined;
    const enumValues = propSchema.enum as unknown[] | undefined;
    const oneOf = propSchema.oneOf as Record<string, unknown>[] | undefined;

    if (enumValues) {
      // Enum -> select
      const select = document.createElement('select');
      select.className =
        'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
      for (const v of enumValues) {
        const opt = document.createElement('option');
        opt.value = String(v);
        opt.textContent = String(v);
        if (String(v) === String(value)) opt.selected = true;
        select.appendChild(opt);
      }
      select.addEventListener('change', () => {
        currentData[key] = select.value;
        onChange(currentData);
      });
      row.appendChild(select);
    } else if (oneOf) {
      // Tagged enum (schemars oneOf) -> select for variant + nested fields
      renderOneOf(row, key, oneOf, value, currentData, onChange);
    } else if (type === 'boolean') {
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = !!value;
      cb.addEventListener('change', () => {
        currentData[key] = cb.checked;
        onChange(currentData);
      });
      row.appendChild(cb);
    } else if (type === 'number' || type === 'integer') {
      const input = document.createElement('input');
      input.type = 'number';
      input.step = type === 'integer' ? '1' : 'any';
      input.value = value != null ? String(value) : '';
      input.className =
        'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
      if (propSchema.minimum != null) input.min = String(propSchema.minimum);
      if (propSchema.maximum != null) input.max = String(propSchema.maximum);
      input.addEventListener('input', () => {
        currentData[key] =
          type === 'integer'
            ? parseInt(input.value) || 0
            : parseFloat(input.value) || 0;
        onChange(currentData);
      });
      row.appendChild(input);
    } else if (type === 'string') {
      const input = document.createElement('input');
      input.type = 'text';
      input.value = value != null ? String(value) : '';
      input.className =
        'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
      input.addEventListener('input', () => {
        currentData[key] = input.value;
        onChange(currentData);
      });
      row.appendChild(input);
    } else if (type === 'object') {
      // Nested object — recurse
      const nested = document.createElement('div');
      nested.className = 'ml-2 border-l border-border pl-2';
      const subData = (value ?? {}) as Record<string, unknown>;
      renderSchemaForm(nested, propSchema, subData, (updated) => {
        currentData[key] = updated;
        onChange(currentData);
      });
      row.appendChild(nested);
    }

    container.appendChild(row);
  }
}

/**
 * Render a oneOf (tagged enum) field.
 *
 * schemars encodes Rust enums with data as `"oneOf": [...]` where each
 * variant is an object with a single `"properties"` entry keyed by the
 * variant name.  Simple unit variants appear as `{ "const": "Name" }` or
 * `{ "enum": ["Name"] }`.
 */
function renderOneOf(
  row: HTMLElement,
  key: string,
  oneOf: Record<string, unknown>[],
  value: unknown,
  currentData: Record<string, unknown>,
  onChange: (data: Record<string, unknown>) => void,
): void {
  // Determine variant names and which one is currently selected.
  const variants: { name: string; schema: Record<string, unknown> | null }[] =
    [];
  for (const variant of oneOf) {
    const name = variantName(variant);
    if (name) {
      const props = variant.properties as
        | Record<string, Record<string, unknown>>
        | undefined;
      variants.push({
        name,
        schema: props?.[name] ?? null,
      });
    }
  }

  const selectedVariant = detectVariant(value, variants);

  // Variant selector
  const select = document.createElement('select');
  select.className =
    'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
  for (const v of variants) {
    const opt = document.createElement('option');
    opt.value = v.name;
    opt.textContent = v.name;
    if (v.name === selectedVariant) opt.selected = true;
    select.appendChild(opt);
  }

  const nestedContainer = document.createElement('div');
  nestedContainer.className = 'ml-2 border-l border-border pl-2 mt-1';

  const renderNested = (variantNameStr: string) => {
    nestedContainer.textContent = '';
    const v = variants.find((vv) => vv.name === variantNameStr);
    if (v?.schema && (v.schema as Record<string, unknown>).type === 'object') {
      const subData =
        typeof value === 'object' && value !== null
          ? ((value as Record<string, unknown>)[variantNameStr] as Record<
              string,
              unknown
            >) ?? {}
          : {};
      renderSchemaForm(nestedContainer, v.schema, subData, (updated) => {
        currentData[key] = { [variantNameStr]: updated };
        onChange(currentData);
      });
    }
  };

  select.addEventListener('change', () => {
    const chosen = select.value;
    const v = variants.find((vv) => vv.name === chosen);
    if (v?.schema) {
      currentData[key] = { [chosen]: {} };
    } else {
      currentData[key] = chosen;
    }
    onChange(currentData);
    renderNested(chosen);
  });

  row.appendChild(select);
  if (selectedVariant) renderNested(selectedVariant);
  if (nestedContainer.childElementCount > 0) {
    row.appendChild(nestedContainer);
  }
}

/** Extract the variant name from a oneOf entry. */
function variantName(variant: Record<string, unknown>): string | null {
  // Unit variant: { "const": "Foo" } or { "enum": ["Foo"] }
  if (typeof variant.const === 'string') return variant.const;
  const enumArr = variant.enum as unknown[] | undefined;
  if (enumArr && enumArr.length === 1 && typeof enumArr[0] === 'string')
    return enumArr[0];
  // Data variant: { "type": "object", "properties": { "Foo": { ... } }, "required": ["Foo"] }
  const props = variant.properties as
    | Record<string, unknown>
    | undefined;
  if (props) {
    const keys = Object.keys(props);
    if (keys.length === 1) return keys[0];
  }
  // Title fallback
  if (typeof variant.title === 'string') return variant.title;
  return null;
}

/** Detect which variant is currently selected from the value. */
function detectVariant(
  value: unknown,
  variants: { name: string; schema: Record<string, unknown> | null }[],
): string | null {
  if (typeof value === 'string') {
    return variants.find((v) => v.name === value)?.name ?? null;
  }
  if (typeof value === 'object' && value !== null) {
    const keys = Object.keys(value as Record<string, unknown>);
    if (keys.length === 1) {
      return variants.find((v) => v.name === keys[0])?.name ?? null;
    }
  }
  return variants.length > 0 ? variants[0].name : null;
}

/**
 * Extract default values from a JSON Schema.
 */
export function schemaDefaults(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  const properties = schema.properties as
    | Record<string, Record<string, unknown>>
    | undefined;
  if (!properties) return {};

  const defaults: Record<string, unknown> = {};
  for (const [key, propSchema] of Object.entries(properties)) {
    if (propSchema.default !== undefined) {
      defaults[key] = propSchema.default;
    } else if (propSchema.type === 'object') {
      defaults[key] = schemaDefaults(propSchema);
    }
  }
  return defaults;
}
