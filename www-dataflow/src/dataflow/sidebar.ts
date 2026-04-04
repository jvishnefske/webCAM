/** Project manager sidebar panel. */

export interface SidebarCallbacks {
  onLoad: (name: string) => void;
  onDelete: (name: string) => void;
}

export interface ProjectInfo {
  name: string;
  lastModified: string;
}

export interface Sidebar {
  element: HTMLDivElement;
  toggle: () => void;
  renderProjects: (projects: ProjectInfo[], activeName: string | null) => void;
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

  function toggle() {
    el.classList.toggle('hidden');
  }

  function renderProjects(projects: ProjectInfo[], activeName: string | null) {
    list.textContent = '';

    if (projects.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'df-project-empty';
      empty.textContent = 'No saved projects';
      list.appendChild(empty);
      return;
    }

    for (const proj of projects) {
      const item = document.createElement('div');
      item.className = 'df-project-item';
      if (proj.name === activeName) item.classList.add('active');

      const info = document.createElement('div');
      info.className = 'df-project-info';
      const nameEl = document.createElement('div');
      nameEl.className = 'df-project-name';
      nameEl.textContent = proj.name;
      info.appendChild(nameEl);
      const dateEl = document.createElement('div');
      dateEl.className = 'df-project-date';
      dateEl.textContent = formatDate(proj.lastModified);
      info.appendChild(dateEl);
      item.appendChild(info);

      const actions = document.createElement('div');
      actions.className = 'df-project-actions';

      const loadBtn = document.createElement('button');
      loadBtn.className = 'df-project-load';
      loadBtn.textContent = 'Load';
      loadBtn.addEventListener('click', () => callbacks.onLoad(proj.name));
      actions.appendChild(loadBtn);

      const deleteBtn = document.createElement('button');
      deleteBtn.className = 'df-project-delete';
      deleteBtn.textContent = 'Delete';
      deleteBtn.addEventListener('click', () => callbacks.onDelete(proj.name));
      actions.appendChild(deleteBtn);

      item.appendChild(actions);
      list.appendChild(item);
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
