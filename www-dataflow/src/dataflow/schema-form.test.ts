import { describe, it, expect, beforeEach } from 'vitest';
import { renderSchemaForm, schemaDefaults } from './schema-form.js';

describe('renderSchemaForm', () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement('div');
  });

  it('renders number input for number type', () => {
    const schema = { properties: { value: { type: 'number', title: 'Value' } } };
    renderSchemaForm(container, schema, { value: 42 }, () => {});
    const input = container.querySelector('input[type="number"]') as HTMLInputElement;
    expect(input).toBeTruthy();
    expect(input.value).toBe('42');
  });

  it('renders text input for string type', () => {
    const schema = { properties: { name: { type: 'string' } } };
    renderSchemaForm(container, schema, { name: 'test' }, () => {});
    const input = container.querySelector('input[type="text"]') as HTMLInputElement;
    expect(input).toBeTruthy();
    expect(input.value).toBe('test');
  });

  it('renders checkbox for boolean type', () => {
    const schema = { properties: { enabled: { type: 'boolean' } } };
    renderSchemaForm(container, schema, { enabled: true }, () => {});
    const cb = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(cb).toBeTruthy();
    expect(cb.checked).toBe(true);
  });

  it('renders select for enum values', () => {
    const schema = { properties: { mode: { enum: ['Fast', 'Slow'] } } };
    renderSchemaForm(container, schema, { mode: 'Fast' }, () => {});
    const select = container.querySelector('select') as HTMLSelectElement;
    expect(select).toBeTruthy();
    expect(select.options.length).toBe(2);
    expect(select.value).toBe('Fast');
  });

  it('renders integer input with step=1', () => {
    const schema = { properties: { count: { type: 'integer' } } };
    renderSchemaForm(container, schema, { count: 5 }, () => {});
    const input = container.querySelector('input[type="number"]') as HTMLInputElement;
    expect(input.step).toBe('1');
  });

  it('calls onChange when value changes', () => {
    let result: Record<string, unknown> = {};
    const schema = { properties: { value: { type: 'number' } } };
    renderSchemaForm(container, schema, { value: 1 }, (d) => { result = d; });
    const input = container.querySelector('input[type="number"]') as HTMLInputElement;
    input.value = '99';
    input.dispatchEvent(new Event('input'));
    expect(result.value).toBe(99);
  });

  it('renders nested objects', () => {
    const schema = {
      properties: {
        nested: {
          type: 'object',
          properties: { x: { type: 'number' } },
        },
      },
    };
    renderSchemaForm(container, schema, { nested: { x: 10 } }, () => {});
    const inputs = container.querySelectorAll('input[type="number"]');
    expect(inputs.length).toBe(1);
  });

  it('handles empty schema', () => {
    renderSchemaForm(container, {}, {}, () => {});
    expect(container.children.length).toBe(0);
  });

  it('uses key as label when title is missing', () => {
    const schema = { properties: { myField: { type: 'string' } } };
    renderSchemaForm(container, schema, {}, () => {});
    const label = container.querySelector('label');
    expect(label).toBeTruthy();
    expect(label!.textContent).toBe('myField');
  });

  it('uses title as label when present', () => {
    const schema = { properties: { x: { type: 'string', title: 'My Label' } } };
    renderSchemaForm(container, schema, {}, () => {});
    const label = container.querySelector('label');
    expect(label!.textContent).toBe('My Label');
  });

  it('respects minimum and maximum on number inputs', () => {
    const schema = { properties: { val: { type: 'number', minimum: 0, maximum: 100 } } };
    renderSchemaForm(container, schema, { val: 50 }, () => {});
    const input = container.querySelector('input[type="number"]') as HTMLInputElement;
    expect(input.min).toBe('0');
    expect(input.max).toBe('100');
  });

  it('calls onChange for boolean toggle', () => {
    let result: Record<string, unknown> = {};
    const schema = { properties: { flag: { type: 'boolean' } } };
    renderSchemaForm(container, schema, { flag: false }, (d) => { result = d; });
    const cb = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    cb.checked = true;
    cb.dispatchEvent(new Event('change'));
    expect(result.flag).toBe(true);
  });

  it('calls onChange for string input', () => {
    let result: Record<string, unknown> = {};
    const schema = { properties: { name: { type: 'string' } } };
    renderSchemaForm(container, schema, { name: '' }, (d) => { result = d; });
    const input = container.querySelector('input[type="text"]') as HTMLInputElement;
    input.value = 'hello';
    input.dispatchEvent(new Event('input'));
    expect(result.name).toBe('hello');
  });

  it('calls onChange for select change', () => {
    let result: Record<string, unknown> = {};
    const schema = { properties: { mode: { enum: ['A', 'B'] } } };
    renderSchemaForm(container, schema, { mode: 'A' }, (d) => { result = d; });
    const select = container.querySelector('select') as HTMLSelectElement;
    select.value = 'B';
    select.dispatchEvent(new Event('change'));
    expect(result.mode).toBe('B');
  });

  it('renders oneOf tagged enum as select', () => {
    const schema = {
      properties: {
        shape: {
          oneOf: [
            { const: 'Circle' },
            { const: 'Square' },
          ],
        },
      },
    };
    renderSchemaForm(container, schema, { shape: 'Circle' }, () => {});
    const select = container.querySelector('select') as HTMLSelectElement;
    expect(select).toBeTruthy();
    expect(select.options.length).toBe(2);
    expect(select.value).toBe('Circle');
  });

  it('clears container before rendering', () => {
    const schema = { properties: { x: { type: 'number' } } };
    renderSchemaForm(container, schema, { x: 1 }, () => {});
    expect(container.querySelectorAll('input').length).toBe(1);
    // Re-render should still have exactly one input
    renderSchemaForm(container, schema, { x: 2 }, () => {});
    expect(container.querySelectorAll('input').length).toBe(1);
  });

  it('renders nested object onChange propagates up', () => {
    let result: Record<string, unknown> = {};
    const schema = {
      properties: {
        nested: {
          type: 'object',
          properties: { y: { type: 'number' } },
        },
      },
    };
    renderSchemaForm(container, schema, { nested: { y: 5 } }, (d) => { result = d; });
    const input = container.querySelector('input[type="number"]') as HTMLInputElement;
    input.value = '20';
    input.dispatchEvent(new Event('input'));
    expect((result.nested as Record<string, unknown>).y).toBe(20);
  });
});

describe('schemaDefaults', () => {
  it('extracts default values', () => {
    const schema = {
      properties: {
        value: { type: 'number', default: 42 },
        name: { type: 'string', default: 'test' },
      },
    };
    expect(schemaDefaults(schema)).toEqual({ value: 42, name: 'test' });
  });

  it('returns empty for no defaults', () => {
    const schema = { properties: { x: { type: 'number' } } };
    expect(schemaDefaults(schema)).toEqual({});
  });

  it('returns empty for schema without properties', () => {
    expect(schemaDefaults({})).toEqual({});
  });

  it('recurses into nested objects', () => {
    const schema = {
      properties: {
        outer: {
          type: 'object',
          properties: {
            inner: { type: 'number', default: 7 },
          },
        },
      },
    };
    expect(schemaDefaults(schema)).toEqual({ outer: { inner: 7 } });
  });
});
