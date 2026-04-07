/** Project manager sidebar panel with hierarchical project+sheet tree. */

export interface SidebarCallbacks {
  onLoadProject: (name: string) => void;
  onDeleteProject: (name: string) => void;
  onSelectSheet: (projectName: string, sheetId: string) => void;
  onAddSheet: (projectName: string) => void;
  onDeleteSheet: (projectName: string, sheetId: string) => void;
}

export interface SheetInfo {
  id: string;
  label: string;
  type: 'dataflow' | 'bsp';
  parentId: string | null;
}

export interface ProjectInfo {
  name: string;
  lastModified: string;
  sheets: SheetInfo[];
}

export interface Sidebar {
  element: HTMLDivElement;
  toggle: () => void;
  renderProjects: (
    projects: ProjectInfo[],
    activeProjectName: string | null,
    activeSheetId: string | null,
  ) => void;
}

export function createSidebar(callbacks: SidebarCallbacks): Sidebar {
  const el = document.createElement('div');
  el.className = 'df-sidebar hidden';

  const header = document.createElement('div');
  header.className = 'df-sidebar-header';
  const title = document.createElement('h3');
  title.textContent = 'Projects';
  header.appendChild(title);
  el.appendChild(header);

  const list = document.createElement('div');
  list.className = 'df-project-list';
  el.appendChild(list);

  const expandedProjects = new Set<string>();

  function toggle() {
    el.classList.toggle('hidden');
  }

  function renderProjects(
    projects: ProjectInfo[],
    activeProjectName: string | null,
    activeSheetId: string | null,
  ) {
    list.textContent = '';

    if (projects.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'df-project-empty';
      empty.textContent = 'No saved projects';
      list.appendChild(empty);
      return;
    }

    // Active project is always expanded
    if (activeProjectName) {
      expandedProjects.add(activeProjectName);
    }

    for (const proj of projects) {
      const isActive = proj.name === activeProjectName;
      const isExpanded = expandedProjects.has(proj.name);

      // Project row
      const item = document.createElement('div');
      item.className = 'df-project-item';
      if (isActive) item.classList.add('active');

      const info = document.createElement('div');
      info.className = 'df-project-info';
      info.style.cursor = 'pointer';
      info.addEventListener('click', () => {
        if (expandedProjects.has(proj.name)) {
          // Don't collapse the active project
          if (!isActive) {
            expandedProjects.delete(proj.name);
          }
        } else {
          expandedProjects.add(proj.name);
        }
        // Load the project when clicking its name
        callbacks.onLoadProject(proj.name);
      });

      const nameEl = document.createElement('div');
      nameEl.className = 'df-project-name';
      const chevron = isExpanded ? '\u25BE' : '\u25B8';
      nameEl.textContent = chevron + ' ' + proj.name;
      info.appendChild(nameEl);

      const dateEl = document.createElement('div');
      dateEl.className = 'df-project-date';
      dateEl.textContent = formatDate(proj.lastModified);
      info.appendChild(dateEl);

      item.appendChild(info);

      const actions = document.createElement('div');
      actions.className = 'df-project-actions';

      const deleteBtn = document.createElement('button');
      deleteBtn.className = 'df-project-delete';
      deleteBtn.textContent = '\u00D7';
      deleteBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        callbacks.onDeleteProject(proj.name);
      });
      actions.appendChild(deleteBtn);

      item.appendChild(actions);
      list.appendChild(item);

      // Sheet list (when expanded)
      if (isExpanded) {
        const sheetList = document.createElement('div');
        sheetList.className = 'df-sheet-list';
        sheetList.style.paddingLeft = '20px';

        for (const sheet of proj.sheets) {
          const sheetItem = document.createElement('div');
          sheetItem.className = 'df-sheet-item';
          if (isActive && sheet.id === activeSheetId) {
            sheetItem.classList.add('active');
          }
          sheetItem.style.display = 'flex';
          sheetItem.style.alignItems = 'center';
          sheetItem.style.justifyContent = 'space-between';
          sheetItem.style.padding = '3px 8px';
          sheetItem.style.fontSize = '11px';
          sheetItem.style.cursor = 'pointer';
          sheetItem.style.borderRadius = '3px';

          sheetItem.addEventListener('click', () => {
            callbacks.onSelectSheet(proj.name, sheet.id);
          });

          const labelSpan = document.createElement('span');
          const isMain = sheet.label.toLowerCase() === 'main';
          if (isActive && sheet.id === activeSheetId) {
            labelSpan.textContent = '\u25CF ' + sheet.label;
          } else {
            labelSpan.textContent = sheet.label;
          }
          sheetItem.appendChild(labelSpan);

          // Type badge
          const badge = document.createElement('span');
          badge.style.fontSize = '9px';
          badge.style.marginLeft = '4px';
          badge.style.opacity = '0.6';
          badge.textContent = sheet.type;
          sheetItem.appendChild(badge);

          // Delete button (not for "main")
          if (!isMain) {
            const sheetDeleteBtn = document.createElement('button');
            sheetDeleteBtn.style.background = 'none';
            sheetDeleteBtn.style.border = 'none';
            sheetDeleteBtn.style.color = 'var(--color-text-dim, #888)';
            sheetDeleteBtn.style.cursor = 'pointer';
            sheetDeleteBtn.style.fontSize = '11px';
            sheetDeleteBtn.style.padding = '0 2px';
            sheetDeleteBtn.textContent = '\u00D7';
            sheetDeleteBtn.addEventListener('click', (e) => {
              e.stopPropagation();
              callbacks.onDeleteSheet(proj.name, sheet.id);
            });
            sheetItem.appendChild(sheetDeleteBtn);
          }

          sheetList.appendChild(sheetItem);
        }

        // Add Sheet button
        const addSheet = document.createElement('div');
        addSheet.className = 'df-sheet-add';
        addSheet.style.padding = '3px 8px';
        addSheet.style.fontSize = '10px';
        addSheet.style.color = 'var(--color-text-dim, #888)';
        addSheet.style.cursor = 'pointer';
        addSheet.textContent = '+ Add Sheet';
        addSheet.addEventListener('click', () => {
          callbacks.onAddSheet(proj.name);
        });
        sheetList.appendChild(addSheet);

        list.appendChild(sheetList);
      }
    }
  }

  return { element: el, toggle, renderProjects };
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    const now = Date.now();
    const diffMs = now - d.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return 'just now';
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return `${diffHr}h ago`;
    const diffDays = Math.floor(diffHr / 24);
    return `${diffDays}d ago`;
  } catch {
    return '';
  }
}
