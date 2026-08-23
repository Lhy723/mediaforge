(() => {
  const menu = document.querySelector('.menu-toggle');
  const nav = document.querySelector('.site-nav');
  if (menu && nav) {
    menu.addEventListener('click', () => {
      const open = nav.classList.toggle('open');
      menu.setAttribute('aria-expanded', String(open));
    });
  }

  const tabs = [...document.querySelectorAll('.code-tab')];
  const panels = [...document.querySelectorAll('.code-panel')];
  tabs.forEach((tab) => tab.addEventListener('click', () => {
    const selected = tab.dataset.tab;
    tabs.forEach((item) => {
      const active = item === tab;
      item.classList.toggle('active', active);
      item.setAttribute('aria-selected', String(active));
    });
    panels.forEach((panel) => panel.classList.toggle('active', panel.dataset.panel === selected));
  }));

  const copyButton = document.querySelector('.copy-button');
  if (copyButton) {
    copyButton.addEventListener('click', async () => {
      const target = document.getElementById(copyButton.dataset.copyTarget);
      if (!target) return;
      try {
        await navigator.clipboard.writeText(target.textContent);
        copyButton.textContent = '已复制';
        window.setTimeout(() => { copyButton.textContent = '复制'; }, 1400);
      } catch {
        copyButton.textContent = '请手动复制';
      }
    });
  }

  const inputs = [document.getElementById('operation-search'), document.getElementById('operation-filter')].filter(Boolean);
  const cards = [...document.querySelectorAll('.operation-card')];
  const empty = document.getElementById('empty-operations');
  const filter = (value) => {
    const query = value.trim().toLowerCase();
    let visible = 0;
    cards.forEach((card) => {
      const match = !query || card.dataset.search.includes(query);
      card.hidden = !match;
      if (match) visible += 1;
    });
    if (empty) empty.hidden = visible !== 0;
  };
  inputs.forEach((input) => input.addEventListener('input', (event) => {
    inputs.forEach((other) => { if (other !== event.target) other.value = event.target.value; });
    filter(event.target.value);
  }));
})();
