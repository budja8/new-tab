// Solo `textContent`: los nombres de sitio vienen de páginas web y nunca deben
// interpretarse como HTML.

const fmt = (n) => n.toLocaleString('es-MX');

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function stat(label, value) {
  const box = el('div', 'stat');
  box.append(el('span', 'stat-value', fmt(value)), el('span', 'stat-label', label));
  return box;
}

function list(title, entries) {
  const section = el('section', 'list');
  section.append(el('h2', '', title));
  if (entries.length === 0) {
    section.append(el('p', 'empty', '—'));
    return section;
  }
  const ul = el('ul');
  for (const [name, n] of entries) {
    const li = el('li');
    li.append(el('span', 'name', name), el('span', 'count', fmt(n)));
    ul.append(li);
  }
  section.append(ul);
  return section;
}

export function renderKpi(root, summary) {
  const row = el('div', 'stats');
  row.append(stat('teclas hoy', summary.total), stat('WPM', summary.wpm));
  root.replaceChildren(
    row,
    list('Teclas', summary.topKeys),
    list('Atajos', summary.topShortcuts),
    list('Sitios', summary.topSites)
  );
}
