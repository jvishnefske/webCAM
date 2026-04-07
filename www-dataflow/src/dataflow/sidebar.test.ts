import { describe, test, expect, beforeEach, vi } from 'vitest';
import { createSidebar, type SidebarCallbacks, type ProjectInfo } from './sidebar.js';

beforeEach(() => {
  document.body.textContent = '';
});

function makeSidebar(overrides: Partial<SidebarCallbacks> = {}) {
  const callbacks: SidebarCallbacks = {
    onLoadProject: overrides.onLoadProject ?? vi.fn(),
    onDeleteProject: overrides.onDeleteProject ?? vi.fn(),
    onSelectSheet: overrides.onSelectSheet ?? vi.fn(),
    onAddSheet: overrides.onAddSheet ?? vi.fn(),
    onDeleteSheet: overrides.onDeleteSheet ?? vi.fn(),
  };
  const sidebar = createSidebar(callbacks);
  document.body.appendChild(sidebar.element);
  return { sidebar, callbacks };
}

function makeProject(name: string, lastModified: string): ProjectInfo {
  return {
    name,
    lastModified,
    sheets: [
      { id: 'main', label: 'Main', type: 'dataflow', parentId: null },
      { id: 'sub1', label: 'Sub Graph', type: 'dataflow', parentId: null },
    ],
  };
}

describe('createSidebar', () => {
  test('creates a sidebar element with project list container', () => {
    const { sidebar } = makeSidebar();
    expect(sidebar.element).toBeInstanceOf(HTMLElement);
    expect(sidebar.element.querySelector('.df-project-list')).not.toBeNull();
  });

  test('sidebar is hidden by default', () => {
    const { sidebar } = makeSidebar();
    expect(sidebar.element.classList.contains('hidden')).toBe(true);
  });

  test('toggle shows and hides', () => {
    const { sidebar } = makeSidebar();
    sidebar.toggle();
    expect(sidebar.element.classList.contains('hidden')).toBe(false);
    sidebar.toggle();
    expect(sidebar.element.classList.contains('hidden')).toBe(true);
  });
});

describe('renderProjects', () => {
  test('renders project list with names and dates', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
      makeProject('Beta', '2026-03-24T10:00:00Z'),
    ], 'Alpha', 'main');

    const items = sidebar.element.querySelectorAll('.df-project-item');
    expect(items.length).toBe(2);
    expect(items[0].textContent).toContain('Alpha');
    expect(items[1].textContent).toContain('Beta');
  });

  test('highlights active project', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
      makeProject('Beta', '2026-03-24T10:00:00Z'),
    ], 'Alpha', 'main');

    const items = sidebar.element.querySelectorAll('.df-project-item');
    expect(items[0].classList.contains('active')).toBe(true);
    expect(items[1].classList.contains('active')).toBe(false);
  });

  test('clicking project info calls onLoadProject with project name', () => {
    const onLoadProject = vi.fn();
    const { sidebar } = makeSidebar({ onLoadProject });
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], null, null);

    const infoEl = sidebar.element.querySelector('.df-project-info') as HTMLElement;
    expect(infoEl).not.toBeNull();
    infoEl.click();
    expect(onLoadProject).toHaveBeenCalledWith('Alpha');
  });

  test('delete button calls onDeleteProject with project name', () => {
    const onDeleteProject = vi.fn();
    const { sidebar } = makeSidebar({ onDeleteProject });
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const deleteBtn = sidebar.element.querySelector('.df-project-delete') as HTMLButtonElement;
    expect(deleteBtn).not.toBeNull();
    deleteBtn.click();
    expect(onDeleteProject).toHaveBeenCalledWith('Alpha');
  });

  test('renders empty state when no projects', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([], null, null);
    expect(sidebar.element.textContent).toContain('No saved projects');
  });

  test('active project shows sheet list expanded', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const sheetList = sidebar.element.querySelector('.df-sheet-list');
    expect(sheetList).not.toBeNull();
    const sheetItems = sheetList!.querySelectorAll('.df-sheet-item');
    expect(sheetItems.length).toBe(2);
  });

  test('inactive project does not show sheet list by default', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], null, null);

    const sheetList = sidebar.element.querySelector('.df-sheet-list');
    expect(sheetList).toBeNull();
  });

  test('active sheet gets active class', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const sheetItems = sidebar.element.querySelectorAll('.df-sheet-item');
    expect(sheetItems[0].classList.contains('active')).toBe(true);
    expect(sheetItems[1].classList.contains('active')).toBe(false);
  });

  test('clicking sheet calls onSelectSheet', () => {
    const onSelectSheet = vi.fn();
    const { sidebar } = makeSidebar({ onSelectSheet });
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const sheetItems = sidebar.element.querySelectorAll('.df-sheet-item');
    (sheetItems[1] as HTMLElement).click();
    expect(onSelectSheet).toHaveBeenCalledWith('Alpha', 'sub1');
  });

  test('"main" sheet has no delete button', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const sheetItems = sidebar.element.querySelectorAll('.df-sheet-item');
    // Main sheet (first) should not have a delete button
    const mainButtons = sheetItems[0].querySelectorAll('button');
    expect(mainButtons.length).toBe(0);
    // Sub sheet (second) should have a delete button
    const subButtons = sheetItems[1].querySelectorAll('button');
    expect(subButtons.length).toBe(1);
  });

  test('sheet delete button calls onDeleteSheet', () => {
    const onDeleteSheet = vi.fn();
    const { sidebar } = makeSidebar({ onDeleteSheet });
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const sheetItems = sidebar.element.querySelectorAll('.df-sheet-item');
    const deleteBtn = sheetItems[1].querySelector('button') as HTMLButtonElement;
    deleteBtn.click();
    expect(onDeleteSheet).toHaveBeenCalledWith('Alpha', 'sub1');
  });

  test('add sheet button calls onAddSheet', () => {
    const onAddSheet = vi.fn();
    const { sidebar } = makeSidebar({ onAddSheet });
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
    ], 'Alpha', 'main');

    const addBtn = sidebar.element.querySelector('.df-sheet-add') as HTMLElement;
    expect(addBtn).not.toBeNull();
    addBtn.click();
    expect(onAddSheet).toHaveBeenCalledWith('Alpha');
  });

  test('chevron indicates expand state', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      makeProject('Alpha', '2026-03-25T10:00:00Z'),
      makeProject('Beta', '2026-03-24T10:00:00Z'),
    ], 'Alpha', 'main');

    const names = sidebar.element.querySelectorAll('.df-project-name');
    // Alpha is active (expanded) -> ▾
    expect(names[0].textContent).toContain('\u25BE');
    // Beta is not expanded -> ▸
    expect(names[1].textContent).toContain('\u25B8');
  });
});
