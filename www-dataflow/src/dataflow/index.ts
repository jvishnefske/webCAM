/** Dataflow mode: wires up the graph manager, editor, and plot display. */

import { dataflow_codegen, dataflow_codegen_multi, dataflow_block_schema } from '../../pkg/rustsim.js';
import { renderSchemaForm } from './schema-form.js';
import { $, $btn, $input } from '../dom.js';
import { DataflowManager } from './graph.js';
import { DataflowEditor } from './editor.js';
import { drawPlot } from './plot.js';
import { getBlockDefaults } from './palette.js';
import { createZip } from './zip.js';
import { HilClient } from './hil-client.js';
import { renderPinTable } from './pin-table.js';
import { renderI2cPanel, updateI2cBuses } from './i2c-panel.js';
import {
  createProject, loadProject, saveProject, deleteProject,
  listProjects, getActiveProjectName, setActiveProjectName,
  uniqueName, createAutoSave, addSheet, removeSheet,
  serializeDataflowSheet,
  type Project, type DataflowSheetData,
} from './storage.js';
import { createSidebar, type ProjectInfo } from './sidebar.js';
import { TelemetryPublisher } from './telemetry.js';
import type { GraphSnapshot, Value } from './types.js';

/** Unwrap a value that may be a plain number or a newtype wrapper {0: number}. */
function unwrapId(v: number | { 0: number }): number {
  return typeof v === 'number' ? v : v[0];
}

let mgr: DataflowManager | null = null;
let editor: DataflowEditor | null = null;
let hilClient: HilClient | null = null;
let activeProject: Project | null = null;
let triggerAutoSave: (() => void) | null = null;
let telemetry: TelemetryPublisher | null = null;

export function initDataflow(): void {
  mgr = new DataflowManager(0.01);
  const container = $('dataflow-workspace') as HTMLDivElement;
  editor = new DataflowEditor(container, mgr);
  telemetry = new TelemetryPublisher();
  mgr.telemetry = telemetry;

  editor.onSelect = (blockId, snap) => {
    updateBlockInfo(blockId, snap);
  };

  editor.onEdgeSelect = (channelId, snap) => {
    updateEdgeInfo(channelId, snap);
  };

  // ── Project management ──────────────────────────────────────────
  const projectNameEl = $('df-project-name');

  /** Save the current active sheet into the active project and persist. */
  function currentSave() {
    if (!activeProject || !mgr || !editor) return;
    const sheetId = activeProject.activeSheet;
    const sheet = activeProject.sheets[sheetId];
    if (sheet?.type === 'dataflow') {
      const snap = mgr.snapshot();
      const viewport = { panX: 0, panY: 0, scale: 1 }; // TODO: expose from editor
      sheet.data = serializeDataflowSheet(snap, mgr.positions, viewport);
    }
    activeProject.lastModified = new Date().toISOString();
    saveProject(activeProject);
    setActiveProjectName(activeProject.name);
  }

  triggerAutoSave = createAutoSave(() => currentSave(), 1000);

  function updateProjectNameDisplay() {
    projectNameEl.textContent = activeProject?.name ?? 'Untitled';
  }

  function refreshSidebar() {
    const names = listProjects();
    const infos: ProjectInfo[] = names.map(name => {
      const p = loadProject(name);
      if (!p) return null;
      return {
        name: p.name,
        lastModified: p.lastModified,
        sheets: Object.values(p.sheets).map(s => ({
          id: s.id, label: s.label, type: s.type, parentId: s.parentId,
        })),
      };
    }).filter((x): x is ProjectInfo => x !== null);
    sidebar.renderProjects(infos, activeProject?.name ?? null, activeProject?.activeSheet ?? null);
  }

  function resetEditor(dt: number) {
    if (!mgr || !editor) return;
    mgr.stop();
    $btn('df-play').textContent = 'Play';
    editor.destroy();
    mgr.destroy();
    mgr = new DataflowManager(dt);
    if (telemetry) mgr.telemetry = telemetry;
    telemetry?.publish({ tag: 55 });
    editor = new DataflowEditor(container, mgr);
    editor.onSelect = (blockId, snap) => updateBlockInfo(blockId, snap);
    editor.onEdgeSelect = (channelId, snap) => updateEdgeInfo(channelId, snap);
    editor.onChange = () => triggerAutoSave?.();
    editor.resize();
  }

  /** Load a dataflow sheet's data into a fresh editor+manager. */
  function loadDataflowSheet(data: DataflowSheetData) {
    const dt = parseFloat($input('df-dt').value) || 0.01;
    resetEditor(dt);
    // Replay blocks
    const idMap = new Map<number, number>();
    for (const block of data.graph.blocks) {
      const newId = mgr!.addBlock(block.blockType, block.config);
      idMap.set(block.id, newId);
      const pos = data.positions[block.id];
      if (pos) mgr!.positions.set(newId, { x: pos.x, y: pos.y });
    }
    // Replay channels
    for (const ch of data.graph.channels) {
      const from = idMap.get(ch.fromBlock);
      const to = idMap.get(ch.toBlock);
      if (from !== undefined && to !== undefined) {
        mgr!.connect(from, ch.fromPort, to, ch.toPort);
      }
    }
    editor!.updateSnapshot();
  }

  /** Switch the active sheet within the current project. */
  function switchToSheet(sheetId: string) {
    if (!activeProject) return;
    currentSave(); // save current sheet first
    activeProject.activeSheet = sheetId;
    const sheet = activeProject.sheets[sheetId];
    if (sheet?.type === 'dataflow') {
      loadDataflowSheet(sheet.data as DataflowSheetData);
    }
    // BSP sheets handled later
    saveProject(activeProject);
    refreshSidebar();
  }

  /** Load an entire project by name, switching to its active sheet. */
  function loadProjectByName(name: string) {
    const project = loadProject(name);
    if (!project) return;
    activeProject = project;
    setActiveProjectName(name);
    const sheet = project.sheets[project.activeSheet];
    if (sheet?.type === 'dataflow') {
      loadDataflowSheet(sheet.data as DataflowSheetData);
    }
    updateProjectNameDisplay();
  }

  // Set up sidebar panel
  const sidebarContainer = $('df-sidebar-panel');
  const sidebar = createSidebar({
    onLoadProject: (name) => {
      currentSave();
      loadProjectByName(name);
      refreshSidebar();
    },
    onDeleteProject: (name) => {
      deleteProject(name);
      if (name === activeProject?.name) {
        const newName = uniqueName('Untitled', listProjects());
        activeProject = createProject(newName);
        saveProject(activeProject);
        setActiveProjectName(newName);
        switchToSheet('main');
        updateProjectNameDisplay();
      }
      refreshSidebar();
    },
    onSelectSheet: (projectName, sheetId) => {
      // If selecting a sheet in a different project, load that project first
      if (projectName !== activeProject?.name) {
        currentSave();
        loadProjectByName(projectName);
      }
      switchToSheet(sheetId);
    },
    onAddSheet: (projectName) => {
      if (projectName !== activeProject?.name) return;
      const label = prompt('Sheet name:');
      if (!label || !label.trim()) return;
      const id = label.trim().toLowerCase().replace(/\s+/g, '-');
      const existingIds = Object.keys(activeProject.sheets);
      const uniqueId = existingIds.includes(id) ? `${id}-${Date.now()}` : id;
      // Default to dataflow; could prompt for type in the future
      addSheet(activeProject, uniqueId, label.trim(), 'dataflow');
      saveProject(activeProject);
      refreshSidebar();
    },
    onDeleteSheet: (projectName, sheetId) => {
      if (projectName !== activeProject?.name) return;
      const wasActive = activeProject.activeSheet === sheetId;
      if (removeSheet(activeProject, sheetId)) {
        saveProject(activeProject);
        if (wasActive) {
          switchToSheet(activeProject.activeSheet);
        }
        refreshSidebar();
      }
    },
  });
  sidebarContainer.appendChild(sidebar.element);

  // Toolbar: New
  $btn('df-new').addEventListener('click', () => {
    currentSave();
    const name = uniqueName('Untitled', listProjects());
    activeProject = createProject(name);
    saveProject(activeProject);
    setActiveProjectName(name);
    switchToSheet('main');
    updateProjectNameDisplay();
    refreshSidebar();
  });

  // Toolbar: Save As
  $btn('df-save-as').addEventListener('click', () => {
    const name = prompt('Project name:');
    if (!name || !name.trim()) return;
    currentSave(); // persist current sheet into activeProject
    const finalName = uniqueName(name.trim(), listProjects());
    // Clone current project under new name
    const cloned: Project = JSON.parse(JSON.stringify(activeProject));
    cloned.name = finalName;
    cloned.lastModified = new Date().toISOString();
    activeProject = cloned;
    saveProject(activeProject);
    setActiveProjectName(finalName);
    updateProjectNameDisplay();
    refreshSidebar();
  });

  // Toolbar: Projects toggle
  $btn('df-projects').addEventListener('click', () => {
    refreshSidebar();
    sidebar.toggle();
  });

  // Auto-save on graph changes
  editor.onChange = () => triggerAutoSave?.();

  // Also auto-save on config apply (handled in updateBlockInfo)

  // Restore last active project on init, or create "Untitled" with main sheet
  const lastActive = getActiveProjectName();
  if (lastActive && loadProject(lastActive)) {
    loadProjectByName(lastActive);
  } else {
    activeProject = createProject('Untitled');
    saveProject(activeProject);
    setActiveProjectName('Untitled');
    updateProjectNameDisplay();
  }

  // Transport controls
  $btn('df-play').addEventListener('click', () => {
    if (!mgr) return;
    if (mgr.running) {
      mgr.stop();
      $btn('df-play').textContent = 'Play';
    } else {
      mgr.start();
      $btn('df-play').textContent = 'Pause';
    }
  });

  $btn('df-reset').addEventListener('click', () => {
    if (!mgr) return;
    const dt = parseFloat($input('df-dt').value) || 0.01;
    resetEditor(dt);
    triggerAutoSave?.();
  });

  $input('df-speed').addEventListener('input', () => {
    if (!mgr) return;
    mgr.setSpeed(parseFloat($input('df-speed').value) || 1.0);
  });

  $btn('df-batch').addEventListener('click', () => {
    if (!mgr) return;
    const steps = parseInt($input('df-batch-steps').value) || 100;
    const dt = parseFloat($input('df-dt').value) || 0.01;
    const snap = mgr.runBatch(steps, dt);
    updatePlots(snap);
    editor?.updateSnapshot();
  });

  // Export Rust crate (legacy single-target)
  $btn('df-export-rust').addEventListener('click', () => {
    if (!mgr) return;
    const statusEl = $('df-export-status');
    const dt = parseFloat($input('df-dt').value) || 0.01;
    try {
      // Check if multi-target checkboxes exist and any are checked
      const targetChecks = document.querySelectorAll<HTMLInputElement>('.df-target-check');
      if (targetChecks.length > 0) {
        const selectedTargets = Array.from(targetChecks)
          .filter(cb => cb.checked)
          .map(cb => ({
            target: cb.dataset.target!,
            binding: { target: cb.dataset.target!, pins: [] },
          }));
        if (selectedTargets.length === 0) {
          statusEl.textContent = 'Select at least one target.';
          statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
          return;
        }
        const json = dataflow_codegen_multi(
          mgr.graphId,
          dt,
          JSON.stringify(selectedTargets),
        );
        const files: Array<[string, string]> = JSON.parse(json);
        downloadAsZip(files);
        statusEl.textContent = `Exported workspace: ${files.length} files.`;
        statusEl.className = 'text-xs mt-2 min-h-4 text-success';
      } else {
        const json = dataflow_codegen(mgr.graphId, dt);
        const files: Array<[string, string]> = JSON.parse(json);
        downloadAsZip(files);
        statusEl.textContent = `Exported ${files.length} files.`;
        statusEl.className = 'text-xs mt-2 min-h-4 text-success';
      }
    } catch (e) {
      statusEl.textContent = `Export error: ${e}`;
      statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
    }
  });

  // Set up target selection checkboxes
  setupTargetCheckboxes();

  // Sidebar block palette
  setupSidebarPalette();

  // HIL connection + tabs
  setupHilConnection();
  setupDfRightTabs();
}

export function resizeDataflow(): void {
  editor?.resize();
}

export function activateDataflow(): void {
  requestAnimationFrame(() => editor?.resize());
}

function updateBlockInfo(blockId: number | null, snap: GraphSnapshot | null): void {
  const infoEl = $('df-block-info');
  if (blockId === null || !snap) {
    // Safe: static content only
    infoEl.textContent = '';
    const span = document.createElement('span');
    span.className = 'text-text-dim';
    span.textContent = 'Select a block to view details';
    infoEl.appendChild(span);
    return;
  }
  const block = snap.blocks.find(b => b.id === blockId);
  if (!block) return;

  // Build info DOM safely
  infoEl.textContent = '';
  const nameEl = document.createElement('b');
  nameEl.textContent = block.name;
  infoEl.appendChild(nameEl);
  const idSpan = document.createElement('span');
  idSpan.className = 'text-text-dim';
  idSpan.textContent = ` #${block.id}`;
  infoEl.appendChild(idSpan);
  infoEl.appendChild(document.createElement('br'));
  const typeSpan = document.createElement('span');
  typeSpan.className = 'text-text-dim text-[11px]';
  typeSpan.textContent = block.block_type;
  infoEl.appendChild(typeSpan);
  infoEl.appendChild(document.createElement('br'));

  if (block.output_values.length > 0) {
    const valDiv = document.createElement('div');
    valDiv.className = 'mt-1.5 text-xs';
    for (let i = 0; i < block.outputs.length; i++) {
      const val = block.output_values[i];
      const row = document.createElement('div');
      row.textContent = `${block.outputs[i].name}: ${formatValue(val)}`;
      valDiv.appendChild(row);
    }
    infoEl.appendChild(valDiv);
  }

  // Config section — schema-driven form
  try {
    const schemaJson = dataflow_block_schema(block.block_type);
    const schema = JSON.parse(schemaJson);
    if (schema.properties) {
      const configDiv = document.createElement('div');
      configDiv.className = 'mt-2 text-xs';
      const configLabel = document.createElement('b');
      configLabel.textContent = 'Config';
      configDiv.appendChild(configLabel);

      const formContainer = document.createElement('div');
      formContainer.className = 'mt-1';

      let currentConfig = { ...(block.config as Record<string, unknown>) };

      renderSchemaForm(formContainer, schema, currentConfig, (updated) => {
        currentConfig = updated;
      });
      configDiv.appendChild(formContainer);

      const applyBtn = document.createElement('button');
      applyBtn.className = 'btn btn-primary btn-sm mt-2';
      applyBtn.textContent = 'Apply';
      applyBtn.addEventListener('click', () => {
        if (!mgr || !editor) return;
        mgr.updateBlock(blockId, block.block_type, currentConfig as Record<string, unknown>);
        const newSnap = mgr.snapshot();
        editor.updateSnapshot();
        updateBlockInfo(blockId, newSnap);
      });
      configDiv.appendChild(applyBtn);
      infoEl.appendChild(configDiv);
    }
  } catch {
    // Fallback: if schema fails, skip config section
  }

  // Delete block button
  const deleteBtn = document.createElement('button');
  deleteBtn.className = 'btn btn-sm mt-2';
  deleteBtn.style.backgroundColor = 'var(--color-danger)';
  deleteBtn.style.color = 'white';
  deleteBtn.textContent = 'Delete Block';
  deleteBtn.addEventListener('click', () => {
    if (!mgr || !editor) return;
    mgr.removeBlock(blockId);
    editor.clearSelection();
    editor.updateSnapshot();
    updateBlockInfo(null, null);
  });
  infoEl.appendChild(deleteBtn);

  // If it's a plot block, update the plot canvas
  updatePlots(snap);
}

function updateEdgeInfo(channelId: number | null, snap: GraphSnapshot | null): void {
  const infoEl = $('df-block-info');
  if (channelId === null || !snap) {
    infoEl.textContent = '';
    const span = document.createElement('span');
    span.className = 'text-text-dim';
    span.textContent = 'Select a block or edge to view details';
    infoEl.appendChild(span);
    return;
  }
  const ch = snap.channels.find(c => unwrapId(c.id) === channelId);
  if (!ch) return;

  const fromBlock = snap.blocks.find(b => b.id === unwrapId(ch.from_block));
  const toBlock = snap.blocks.find(b => b.id === unwrapId(ch.to_block));

  infoEl.textContent = '';
  const title = document.createElement('b');
  title.textContent = 'Channel';
  infoEl.appendChild(title);
  const idSpan = document.createElement('span');
  idSpan.className = 'text-text-dim';
  idSpan.textContent = ` #${channelId}`;
  infoEl.appendChild(idSpan);
  infoEl.appendChild(document.createElement('br'));

  const detailDiv = document.createElement('div');
  detailDiv.className = 'mt-1.5 text-xs';

  const fromName = fromBlock ? `${fromBlock.name}` : `Block ${unwrapId(ch.from_block)}`;
  const fromPortName = fromBlock?.outputs[ch.from_port]?.name ?? `port ${ch.from_port}`;
  const toName = toBlock ? `${toBlock.name}` : `Block ${unwrapId(ch.to_block)}`;
  const toPortName = toBlock?.inputs[ch.to_port]?.name ?? `port ${ch.to_port}`;

  const fromRow = document.createElement('div');
  fromRow.textContent = `From: ${fromName} \u2192 ${fromPortName}`;
  detailDiv.appendChild(fromRow);

  const toRow = document.createElement('div');
  toRow.textContent = `To: ${toName} \u2192 ${toPortName}`;
  detailDiv.appendChild(toRow);

  infoEl.appendChild(detailDiv);

  const disconnectBtn = document.createElement('button');
  disconnectBtn.className = 'btn btn-sm mt-2';
  disconnectBtn.style.backgroundColor = 'var(--color-danger)';
  disconnectBtn.style.color = 'white';
  disconnectBtn.textContent = 'Disconnect';
  disconnectBtn.addEventListener('click', () => {
    if (!mgr || !editor) return;
    mgr.disconnect(channelId);
    editor.clearSelection();
    editor.updateSnapshot();
    updateEdgeInfo(null, null);
  });
  infoEl.appendChild(disconnectBtn);
}

function formatValue(val: Value | null): string {
  if (!val) return '\u2014';
  switch (val.type) {
    case 'Float': return val.data.toFixed(4);
    case 'Text': return `"${val.data.slice(0, 30)}"`;
    case 'Bytes': return `[${val.data.length} bytes]`;
    case 'Series': return `[${val.data.length} samples]`;
  }
}

// ── Sidebar block palette ─────────────────────────────────────────

function setupSidebarPalette(): void {
  const containerEl = document.getElementById('df-sidebar-palette');
  const filterInput = document.getElementById('df-palette-filter') as HTMLInputElement | null;
  if (!containerEl) return;
  const container = containerEl;

  const blockTypes = DataflowManager.blockTypes();

  function render(filter: string): void {
    container.textContent = '';
    const lower = filter.toLowerCase();
    let lastCat = '';

    for (const bt of blockTypes) {
      if (filter && !bt.name.toLowerCase().includes(lower) && !bt.block_type.toLowerCase().includes(lower)) {
        continue;
      }
      if (bt.category !== lastCat) {
        lastCat = bt.category;
        const header = document.createElement('div');
        header.className = 'text-[10px] text-text-dim uppercase tracking-wider px-2 pt-2 pb-1';
        if (container.childNodes.length > 0) {
          header.style.borderTop = '1px solid var(--color-border)';
        }
        header.textContent = bt.category;
        container.appendChild(header);
      }
      const item = document.createElement('button');
      item.className = 'block w-full text-left text-xs px-2 py-1.5 cursor-pointer bg-transparent border-none text-text transition-colors';
      item.style.cssText = 'border-left: 2px solid transparent;';
      item.addEventListener('mouseenter', () => {
        item.style.background = 'var(--color-border)';
        item.style.borderLeftColor = 'var(--color-accent)';
      });
      item.addEventListener('mouseleave', () => {
        item.style.background = 'transparent';
        item.style.borderLeftColor = 'transparent';
      });
      item.textContent = bt.name;
      item.addEventListener('click', () => {
        if (!mgr || !editor) return;
        const config = getBlockDefaults(bt.block_type);
        mgr.addBlock(bt.block_type, config, 200, 200);
        editor.updateSnapshot();
        editor.onChange?.();
      });
      container.appendChild(item);
    }
  }

  render('');
  filterInput?.addEventListener('input', () => render(filterInput.value));
}

const TARGET_OPTIONS = [
  { id: 'Host', label: 'Host (Simulation)', checked: true },
  { id: 'Rp2040', label: 'RP2040 (Pico)', checked: false },
  { id: 'Stm32f4', label: 'STM32F4', checked: false },
  { id: 'Esp32c3', label: 'ESP32-C3', checked: false },
] as const;

function setupTargetCheckboxes(): void {
  const container = document.getElementById('df-target-select');
  if (!container) return;

  container.textContent = '';
  const label = document.createElement('b');
  label.textContent = 'Targets';
  label.className = 'text-xs';
  container.appendChild(label);

  for (const target of TARGET_OPTIONS) {
    const row = document.createElement('label');
    row.className = 'flex items-center gap-1.5 text-xs mt-1 cursor-pointer';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = target.checked;
    cb.className = 'df-target-check';
    cb.dataset.target = target.id;
    row.appendChild(cb);
    const span = document.createElement('span');
    span.textContent = target.label;
    row.appendChild(span);
    container.appendChild(row);
  }
}

// ── HIL connection ─────────────────────────────────────────────────

function setupHilConnection(): void {
  const connectBtn = $btn('hil-connect');
  const statusEl = $('hil-status');
  const deployBtn = document.getElementById('hil-deploy') as HTMLButtonElement;
  const deployStatus = $('hil-deploy-status');
  const pinContainer = $('hil-pin-table');
  const i2cContainer = $('hil-i2c-panel');

  connectBtn.addEventListener('click', () => {
    if (hilClient?.connected) {
      hilClient.disconnect();
      return;
    }
    const url = $input('hil-ws-url').value.trim();
    if (!url) return;

    hilClient = new HilClient();

    hilClient.onConnect = () => {
      statusEl.textContent = 'Connected';
      statusEl.className = 'text-xs text-success mb-2';
      connectBtn.textContent = 'Disconnect';
      deployBtn.disabled = false;
      if (telemetry && hilClient?.socket) {
        telemetry.attach(hilClient.socket);
        telemetry.setEnabled(true);
      }
    };

    hilClient.onDisconnect = () => {
      statusEl.textContent = 'Disconnected';
      statusEl.className = 'text-xs text-text-dim mb-2';
      connectBtn.textContent = 'Connect';
      deployBtn.disabled = true;
      telemetry?.detach();
    };

    hilClient.onBusList = (buses) => {
      updateI2cBuses(i2cContainer, buses, hilClient!);
    };

    hilClient.onPinConfig = (pins) => {
      renderPinTable(pinContainer, pins, (_idx, _pin) => {
        // Send pin edit back to MCU (future: dedicated set-pin message)
        hilClient?.getPinConfig();
      });
    };

    hilClient.onError = (msg) => {
      deployStatus.textContent = `Error: ${msg}`;
      deployStatus.className = 'text-xs mt-1 min-h-4 text-danger';
    };

    hilClient.onDeployAck = () => {
      deployStatus.textContent = 'Deployed successfully.';
      deployStatus.className = 'text-xs mt-1 min-h-4 text-success';
    };

    // Render initial empty I2C panel
    renderI2cPanel(i2cContainer, hilClient);

    hilClient.connect(url);
    statusEl.textContent = 'Connecting\u2026';
    statusEl.className = 'text-xs text-warning mb-2';
  });

  // Deploy button
  deployBtn.addEventListener('click', () => {
    if (!mgr || !hilClient?.connected) return;
    const dt = parseFloat($input('df-dt').value) || 0.01;

    // Get selected target
    const targetChecks = document.querySelectorAll<HTMLInputElement>('.df-target-check:checked');
    const target = targetChecks.length > 0 ? targetChecks[0].dataset.target! : 'Host';

    try {
      const snap = mgr.snapshot();
      const snapshotJson = JSON.stringify(snap);
      hilClient.deploy(snapshotJson, target, dt);
      deployStatus.textContent = 'Deploying\u2026';
      deployStatus.className = 'text-xs mt-1 min-h-4 text-warning';
    } catch (e) {
      deployStatus.textContent = `Deploy error: ${e}`;
      deployStatus.className = 'text-xs mt-1 min-h-4 text-danger';
    }
  });
}

// ── Right-pane tab switching ───────────────────────────────────────

function setupDfRightTabs(): void {
  const tabs = document.querySelectorAll<HTMLButtonElement>('.df-right-tab');
  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      const targetId = tab.dataset.dftab;
      if (!targetId) return;

      // Deactivate all tabs and panes
      tabs.forEach(t => t.classList.remove('active'));
      document.querySelectorAll('.df-tab-content').forEach(p => p.classList.remove('active'));

      // Activate clicked
      tab.classList.add('active');
      document.getElementById(targetId)?.classList.add('active');
    });
  });
}

/** Download generated crate files as a ZIP archive. */
function downloadAsZip(files: Array<[string, string]>): void {
  const blob = createZip(files);
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'dataflow-generated.zip';
  a.click();
  URL.revokeObjectURL(a.href);
}

function updatePlots(snap: GraphSnapshot): void {
  const plotCanvas = document.getElementById('df-plot-canvas') as HTMLCanvasElement | null;
  if (!plotCanvas) return;

  // Find the first plot block with series data
  for (const block of snap.blocks) {
    if (block.block_type === 'plot') {
      const val = block.output_values[0];
      if (val && val.type === 'Series') {
        drawPlot(plotCanvas, val.data, `Plot #${block.id}`);
        return;
      }
    }
  }
}
