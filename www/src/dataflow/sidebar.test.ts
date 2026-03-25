import { describe, test, expect, beforeEach, vi } from 'vitest';
import { createSidebar, type SidebarCallbacks } from './sidebar.js';

beforeEach(() => {
  document.body.textContent = '';
});

function makeSidebar(overrides: Partial<SidebarCallbacks> = {}) {
  const callbacks: SidebarCallbacks = {
    onLoad: overrides.onLoad ?? vi.fn(),
    onDelete: overrides.onDelete ?? vi.fn(),
  };
  const sidebar = createSidebar(callbacks);
  document.body.appendChild(sidebar.element);
  return { sidebar, callbacks };
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
      { name: 'Alpha', lastModified: '2026-03-25T10:00:00Z' },
      { name: 'Beta', lastModified: '2026-03-24T10:00:00Z' },
    ], 'Alpha');

    const items = sidebar.element.querySelectorAll('.df-project-item');
    expect(items.length).toBe(2);
    expect(items[0].textContent).toContain('Alpha');
    expect(items[1].textContent).toContain('Beta');
  });

  test('highlights active project', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([
      { name: 'Alpha', lastModified: '2026-03-25T10:00:00Z' },
      { name: 'Beta', lastModified: '2026-03-24T10:00:00Z' },
    ], 'Alpha');

    const items = sidebar.element.querySelectorAll('.df-project-item');
    expect(items[0].classList.contains('active')).toBe(true);
    expect(items[1].classList.contains('active')).toBe(false);
  });

  test('load button calls onLoad with project name', () => {
    const onLoad = vi.fn();
    const { sidebar } = makeSidebar({ onLoad });
    sidebar.renderProjects([
      { name: 'Alpha', lastModified: '2026-03-25T10:00:00Z' },
    ], null);

    const loadBtn = sidebar.element.querySelector('.df-project-load') as HTMLButtonElement;
    expect(loadBtn).not.toBeNull();
    loadBtn.click();
    expect(onLoad).toHaveBeenCalledWith('Alpha');
  });

  test('delete button calls onDelete with project name', () => {
    const onDelete = vi.fn();
    const { sidebar } = makeSidebar({ onDelete });
    sidebar.renderProjects([
      { name: 'Alpha', lastModified: '2026-03-25T10:00:00Z' },
    ], null);

    const deleteBtn = sidebar.element.querySelector('.df-project-delete') as HTMLButtonElement;
    expect(deleteBtn).not.toBeNull();
    deleteBtn.click();
    expect(onDelete).toHaveBeenCalledWith('Alpha');
  });

  test('renders empty state when no projects', () => {
    const { sidebar } = makeSidebar();
    sidebar.renderProjects([], null);
    expect(sidebar.element.textContent).toContain('No saved projects');
  });
});
