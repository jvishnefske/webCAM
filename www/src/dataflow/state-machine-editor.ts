/** Structured form editor for state machine block configuration. */

import type { DataflowManager } from './graph.js';
import type {
  StateMachineConfig, TopicBinding, TransitionConfig,
  FieldCondition, MessageSchema, FieldType,
} from './types.js';

const FIELD_TYPES: FieldType[] = ['F32', 'F64', 'U8', 'U16', 'U32', 'I32', 'Bool'];

// -- DOM helpers --

function el(tag: string, cls: string): HTMLElement {
  const e = document.createElement(tag);
  e.className = cls;
  return e;
}

function inputEl(value: string, onChange: (v: string) => void): HTMLInputElement {
  const inp = document.createElement('input');
  inp.type = 'text';
  inp.className = 'sm-input';
  inp.value = value;
  inp.addEventListener('input', () => onChange(inp.value));
  return inp;
}

function deleteBtn(onClick: () => void): HTMLButtonElement {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'sm-delete-btn';
  btn.textContent = 'x';
  btn.addEventListener('click', onClick);
  return btn;
}

function addBtn(label: string, onClick: () => void): HTMLButtonElement {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'sm-add-btn';
  btn.textContent = label;
  btn.addEventListener('click', onClick);
  return btn;
}

function sectionHeader(title: string): HTMLElement {
  const div = el('div', 'sm-section-header');
  div.textContent = title;
  return div;
}

function clearBody(section: HTMLElement): HTMLElement {
  const existing = section.querySelector('.sm-section-body');
  if (existing) existing.remove();
  const body = el('div', 'sm-section-body');
  section.appendChild(body);
  return body;
}

function stateSelect(
  states: string[],
  selected: string,
  onChange: (v: string) => void,
): HTMLSelectElement {
  const sel = document.createElement('select');
  sel.className = 'sm-select';
  for (const s of states) {
    const opt = document.createElement('option');
    opt.value = s;
    opt.textContent = s;
    opt.selected = s === selected;
    sel.appendChild(opt);
  }
  sel.addEventListener('change', () => onChange(sel.value));
  return sel;
}

// -- Commit helper --

function commit(
  blockId: number,
  cfg: StateMachineConfig,
  mgr: DataflowManager,
  onConfigChanged: () => void,
): void {
  mgr.updateBlock(blockId, 'state_machine', cfg as unknown as Record<string, unknown>);
  onConfigChanged();
}

// -- Section: States --

function renderStates(
  section: HTMLElement,
  blockId: number,
  cfg: StateMachineConfig,
  mgr: DataflowManager,
  onConfigChanged: () => void,
  rerender: () => void,
): void {
  const body = clearBody(section);

  for (let i = 0; i < cfg.states.length; i++) {
    const state = cfg.states[i];
    const row = el('div', 'sm-row') as HTMLDivElement;

    // Radio for initial state
    const radio = document.createElement('input');
    radio.type = 'radio';
    radio.name = `sm-initial-${blockId}`;
    radio.checked = state === cfg.initial;
    const radioLabel = el('label', 'sm-radio-label') as HTMLLabelElement;
    radioLabel.appendChild(radio);
    radio.addEventListener('change', () => {
      if (radio.checked) {
        cfg.initial = state;
        commit(blockId, cfg, mgr, onConfigChanged);
      }
    });
    row.appendChild(radioLabel);

    // Name input
    const nameInp = inputEl(state, (v) => {
      const oldName = cfg.states[i];
      cfg.states[i] = v;
      // Update initial if it pointed to this state
      if (cfg.initial === oldName) cfg.initial = v;
      // Update transitions
      for (const t of cfg.transitions) {
        if (t.from === oldName) t.from = v;
        if (t.to === oldName) t.to = v;
      }
      commit(blockId, cfg, mgr, onConfigChanged);
    });
    row.appendChild(nameInp);

    // Delete button (disabled if only 1 state)
    const del = deleteBtn(() => {
      const name = cfg.states[i];
      cfg.states.splice(i, 1);
      // Fix initial
      if (cfg.initial === name && cfg.states.length > 0) cfg.initial = cfg.states[0];
      // Remove transitions referencing this state
      cfg.transitions = cfg.transitions.filter(t => t.from !== name && t.to !== name);
      commit(blockId, cfg, mgr, onConfigChanged);
      rerender();
    });
    del.disabled = cfg.states.length <= 1;
    row.appendChild(del);

    body.appendChild(row);
  }

  body.appendChild(addBtn('Add State', () => {
    cfg.states.push(`state_${cfg.states.length}`);
    commit(blockId, cfg, mgr, onConfigChanged);
    rerender();
  }));
}

// -- Field sub-editor (shared by input/output topics) --

function renderFields(
  container: HTMLElement,
  schema: MessageSchema,
  onChanged: () => void,
): void {
  const fieldsDiv = el('div', 'sm-fields');

  for (let fi = 0; fi < schema.fields.length; fi++) {
    const field = schema.fields[fi];
    const row = el('div', 'sm-field-row');

    // Field name
    const nameInp = inputEl(field.name, (v) => {
      schema.fields[fi].name = v;
      onChanged();
    });
    nameInp.className = 'sm-input sm-input-sm';
    row.appendChild(nameInp);

    // Field type dropdown
    const typeSel = document.createElement('select');
    typeSel.className = 'sm-select sm-select-sm';
    for (const ft of FIELD_TYPES) {
      const opt = document.createElement('option');
      opt.value = ft;
      opt.textContent = ft;
      opt.selected = ft === field.field_type;
      typeSel.appendChild(opt);
    }
    typeSel.addEventListener('change', () => {
      schema.fields[fi].field_type = typeSel.value as FieldType;
      onChanged();
    });
    row.appendChild(typeSel);

    // Delete field
    row.appendChild(deleteBtn(() => {
      schema.fields.splice(fi, 1);
      onChanged();
    }));

    fieldsDiv.appendChild(row);
  }

  // Add field button
  fieldsDiv.appendChild(addBtn('Add field', () => {
    schema.fields.push({ name: `field_${schema.fields.length}`, field_type: 'F32' });
    onChanged();
  }));

  container.appendChild(fieldsDiv);
}

// -- Section: Topics (shared for input/output) --

function renderTopics(
  section: HTMLElement,
  blockId: number,
  cfg: StateMachineConfig,
  topics: TopicBinding[],
  mgr: DataflowManager,
  onConfigChanged: () => void,
  rerender: () => void,
): void {
  const body = clearBody(section);

  for (let i = 0; i < topics.length; i++) {
    const topic = topics[i];
    const topicRow = el('div', 'sm-topic-row');

    // Topic name input
    const nameInp = inputEl(topic.topic, (v) => {
      topics[i].topic = v;
      commit(blockId, cfg, mgr, onConfigChanged);
    });
    topicRow.appendChild(nameInp);

    // Delete topic
    topicRow.appendChild(deleteBtn(() => {
      topics.splice(i, 1);
      commit(blockId, cfg, mgr, onConfigChanged);
      rerender();
    }));

    body.appendChild(topicRow);

    // Field sub-editor inline
    renderFields(body, topic.schema, () => {
      commit(blockId, cfg, mgr, onConfigChanged);
      rerender();
    });
  }

  body.appendChild(addBtn('Add Topic', () => {
    topics.push({ topic: `topic_${topics.length}`, schema: { name: `Schema${topics.length}`, fields: [] } });
    commit(blockId, cfg, mgr, onConfigChanged);
    rerender();
  }));
}

// -- Section: Transitions --

function renderTransitions(
  section: HTMLElement,
  blockId: number,
  cfg: StateMachineConfig,
  mgr: DataflowManager,
  onConfigChanged: () => void,
  rerender: () => void,
): void {
  const body = clearBody(section);
  const states = cfg.states;
  const inputTopics = cfg.input_topics;

  for (let i = 0; i < cfg.transitions.length; i++) {
    const t = cfg.transitions[i];
    const row = el('div', 'sm-transition-row');

    // From state dropdown
    row.appendChild(stateSelect(states, t.from, (v) => {
      cfg.transitions[i].from = v;
      commit(blockId, cfg, mgr, onConfigChanged);
    }));

    const arrow = el('span', '');
    arrow.textContent = ' → ';
    row.appendChild(arrow);

    // To state dropdown
    row.appendChild(stateSelect(states, t.to, (v) => {
      cfg.transitions[i].to = v;
      commit(blockId, cfg, mgr, onConfigChanged);
    }));

    // Guard type dropdown
    const guardSel = document.createElement('select');
    guardSel.className = 'sm-select';
    for (const gt of ['Unconditional', 'Topic']) {
      const opt = document.createElement('option');
      opt.value = gt;
      opt.textContent = gt;
      opt.selected = t.guard.type === gt;
      guardSel.appendChild(opt);
    }
    guardSel.addEventListener('change', () => {
      if (guardSel.value === 'Unconditional') {
        cfg.transitions[i].guard = { type: 'Unconditional' };
      } else {
        const firstTopic = inputTopics[0]?.topic ?? '';
        cfg.transitions[i].guard = { type: 'Topic', topic: firstTopic };
      }
      commit(blockId, cfg, mgr, onConfigChanged);
      rerender();
    });
    row.appendChild(guardSel);

    // Topic guard details
    if (t.guard.type === 'Topic') {
      const guard = t.guard as { type: 'Topic'; topic: string; condition?: FieldCondition };

      // Topic dropdown
      const topicSel = document.createElement('select');
      topicSel.className = 'sm-select';
      for (const tb of inputTopics) {
        const opt = document.createElement('option');
        opt.value = tb.topic;
        opt.textContent = tb.topic;
        opt.selected = tb.topic === guard.topic;
        topicSel.appendChild(opt);
      }
      topicSel.addEventListener('change', () => {
        (cfg.transitions[i].guard as { type: 'Topic'; topic: string }).topic = topicSel.value;
        commit(blockId, cfg, mgr, onConfigChanged);
      });
      row.appendChild(topicSel);

      // Condition block
      const condDiv = el('div', 'sm-condition');

      // Checkbox to enable condition
      const condCheck = document.createElement('input');
      condCheck.type = 'checkbox';
      condCheck.checked = guard.condition !== undefined;
      condDiv.appendChild(condCheck);

      if (guard.condition !== undefined) {
        const cond = guard.condition;

        // Resolve topic schema for field dropdown
        const topicBinding = inputTopics.find(tb => tb.topic === guard.topic);
        const fields = topicBinding?.schema.fields ?? [];

        // Field dropdown
        const fieldSel = document.createElement('select');
        fieldSel.className = 'sm-select sm-select-sm';
        for (const f of fields) {
          const opt = document.createElement('option');
          opt.value = f.name;
          opt.textContent = f.name;
          opt.selected = f.name === cond.field;
          fieldSel.appendChild(opt);
        }
        fieldSel.addEventListener('change', () => {
          (cfg.transitions[i].guard as { type: 'Topic'; topic: string; condition: FieldCondition })
            .condition.field = fieldSel.value;
          commit(blockId, cfg, mgr, onConfigChanged);
        });
        condDiv.appendChild(fieldSel);

        // Op dropdown
        const opSel = document.createElement('select');
        opSel.className = 'sm-select sm-select-sm';
        for (const op of ['Eq', 'Ne', 'Gt', 'Lt', 'Ge', 'Le'] as const) {
          const opt = document.createElement('option');
          opt.value = op;
          opt.textContent = op;
          opt.selected = op === cond.op;
          opSel.appendChild(opt);
        }
        opSel.addEventListener('change', () => {
          (cfg.transitions[i].guard as { type: 'Topic'; topic: string; condition: FieldCondition })
            .condition.op = opSel.value as FieldCondition['op'];
          commit(blockId, cfg, mgr, onConfigChanged);
        });
        condDiv.appendChild(opSel);

        // Value number input
        const valInp = document.createElement('input');
        valInp.type = 'number';
        valInp.className = 'sm-input sm-input-sm';
        valInp.value = String(cond.value);
        valInp.addEventListener('input', () => {
          (cfg.transitions[i].guard as { type: 'Topic'; topic: string; condition: FieldCondition })
            .condition.value = parseFloat(valInp.value) || 0;
          commit(blockId, cfg, mgr, onConfigChanged);
        });
        condDiv.appendChild(valInp);
      }

      condCheck.addEventListener('change', () => {
        if (condCheck.checked) {
          const topicBinding = inputTopics.find(tb => tb.topic === guard.topic);
          const firstField = topicBinding?.schema.fields[0]?.name ?? '';
          (cfg.transitions[i].guard as { type: 'Topic'; topic: string; condition?: FieldCondition })
            .condition = { field: firstField, op: 'Eq', value: 0 };
        } else {
          delete (cfg.transitions[i].guard as { type: 'Topic'; topic: string; condition?: FieldCondition })
            .condition;
        }
        commit(blockId, cfg, mgr, onConfigChanged);
        rerender();
      });

      row.appendChild(condDiv);
    }

    // Delete transition
    row.appendChild(deleteBtn(() => {
      cfg.transitions.splice(i, 1);
      commit(blockId, cfg, mgr, onConfigChanged);
      rerender();
    }));

    body.appendChild(row);
  }

  // Add transition button
  body.appendChild(addBtn('Add Transition', () => {
    const from = states[0] ?? '';
    const to = states[1] ?? states[0] ?? '';
    const newT: TransitionConfig = {
      from,
      to,
      guard: { type: 'Unconditional' },
      actions: [],
    };
    cfg.transitions.push(newT);
    commit(blockId, cfg, mgr, onConfigChanged);
    rerender();
  }));
}

// -- Main mount function --

export function mountStateMachineEditor(
  container: HTMLElement,
  blockId: number,
  config: StateMachineConfig,
  mgr: DataflowManager,
  onConfigChanged: () => void,
): void {
  // Work on a deep copy so mutations don't alias the snapshot
  const cfg = structuredClone(config);

  const editorDiv = el('div', 'sm-editor');
  container.appendChild(editorDiv);

  // Build sections
  const statesSection = el('div', 'sm-section');
  statesSection.appendChild(sectionHeader('States'));
  editorDiv.appendChild(statesSection);

  const inputTopicsSection = el('div', 'sm-section');
  inputTopicsSection.appendChild(sectionHeader('Input Topics'));
  editorDiv.appendChild(inputTopicsSection);

  const outputTopicsSection = el('div', 'sm-section');
  outputTopicsSection.appendChild(sectionHeader('Output Topics'));
  editorDiv.appendChild(outputTopicsSection);

  const transitionsSection = el('div', 'sm-section');
  transitionsSection.appendChild(sectionHeader('Transitions'));
  editorDiv.appendChild(transitionsSection);

  // Rerender helpers — each section rerenders itself when its data changes
  function rerenderStates(): void {
    renderStates(statesSection, blockId, cfg, mgr, onConfigChanged, rerenderStates);
    // Transitions reference states, so rerender those too
    renderTransitions(transitionsSection, blockId, cfg, mgr, onConfigChanged, rerenderTransitions);
  }

  function rerenderInputTopics(): void {
    renderTopics(
      inputTopicsSection, blockId, cfg, cfg.input_topics,
      mgr, onConfigChanged, rerenderInputTopics,
    );
    // Transitions reference input topics
    renderTransitions(transitionsSection, blockId, cfg, mgr, onConfigChanged, rerenderTransitions);
  }

  function rerenderOutputTopics(): void {
    renderTopics(
      outputTopicsSection, blockId, cfg, cfg.output_topics,
      mgr, onConfigChanged, rerenderOutputTopics,
    );
  }

  function rerenderTransitions(): void {
    renderTransitions(transitionsSection, blockId, cfg, mgr, onConfigChanged, rerenderTransitions);
  }

  // Initial render
  rerenderStates();
  rerenderInputTopics();
  rerenderOutputTopics();
  rerenderTransitions();
}
