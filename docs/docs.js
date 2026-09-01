(() => {
  const root = document.documentElement;
  const languageToggle = document.getElementById('language-toggle');
  const menu = document.querySelector('.menu-toggle');
  const nav = document.querySelector('.site-nav');
  const navLinks = nav ? [...nav.querySelectorAll('a[href^="#"]')] : [];
  const navSections = navLinks
    .map((link) => document.getElementById(link.getAttribute('href').slice(1)))
    .filter(Boolean);
  const copyButton = document.getElementById('copy-button');
  const copyLabel = document.getElementById('copy-label');
  const operationFilter = document.getElementById('operation-filter');
  const cards = [...document.querySelectorAll('.operation-card')];
  const empty = document.getElementById('empty-operations');
  const langStorageKey = 'mediaforge-docs-language';

  const copyText = {
    zh: { idle: '复制', done: '已复制', failed: '请手动复制' },
    en: { idle: 'Copy', done: 'Copied', failed: 'Copy manually' },
  };

  let activeNavLink = navLinks.find((link) => link.classList.contains('active')) || navLinks[0];

  const moveNavIndicator = () => {
    if (!nav || !activeNavLink) return;
    nav.style.setProperty('--nav-indicator-x', `${activeNavLink.offsetLeft}px`);
    nav.style.setProperty('--nav-indicator-width', `${activeNavLink.offsetWidth}px`);
  };

  const setActiveNav = (sectionId) => {
    const nextLink = navLinks.find((link) => link.getAttribute('href') === `#${sectionId}`);
    if (!nextLink) return;
    activeNavLink = nextLink;
    navLinks.forEach((link) => {
      const active = link === nextLink;
      link.classList.toggle('active', active);
      if (active) link.setAttribute('aria-current', 'page');
      else link.removeAttribute('aria-current');
    });
    moveNavIndicator();
  };

  const setCopyLabel = (state = 'idle') => {
    if (!copyLabel) return;
    const lang = root.dataset.lang === 'en' ? 'en' : 'zh';
    copyLabel.textContent = copyText[lang][state];
  };

  const setLanguage = (lang) => {
    const next = lang === 'en' ? 'en' : 'zh';
    root.dataset.lang = next;
    root.lang = next === 'en' ? 'en' : 'zh-CN';

    const meta = document.querySelector('meta[name="description"]');
    if (meta) {
      meta.content = next === 'en'
        ? 'MediaForge: a verifiable media processing tool for AI agents.'
        : 'MediaForge：面向 AI Agent 的可验证媒体处理工具。';
    }
    document.title = next === 'en'
      ? 'MediaForge | Verifiable media tooling'
      : 'MediaForge | 可验证的媒体处理工具';

    if (operationFilter) {
      const placeholder = operationFilter.dataset[next === 'en' ? 'placeholderEn' : 'placeholderZh'];
      if (placeholder) operationFilter.placeholder = placeholder;
    }
    if (languageToggle) {
      languageToggle.setAttribute('aria-label', next === 'en' ? '切换为中文' : 'Switch to English');
      languageToggle.setAttribute('aria-pressed', String(next === 'en'));
    }
    if (menu) menu.setAttribute('aria-label', next === 'en' ? 'Open menu' : '打开菜单');
    setCopyLabel(copyButton?.dataset.copyState || 'idle');

    try { window.localStorage.setItem(langStorageKey, next); } catch { /* Storage can be unavailable in private contexts. */ }
  };

  let savedLanguage = 'zh';
  try {
    savedLanguage = window.localStorage.getItem(langStorageKey) || 'zh';
  } catch { /* Use Chinese as the stable default when storage is unavailable. */ }
  setLanguage(savedLanguage);
  setActiveNav(window.location.hash.slice(1) || 'top');
  if (window.requestAnimationFrame) window.requestAnimationFrame(moveNavIndicator);
  else moveNavIndicator();

  navLinks.forEach((link) => link.addEventListener('click', () => {
    setActiveNav(link.getAttribute('href').slice(1));
  }));

  window.addEventListener('hashchange', () => {
    setActiveNav(window.location.hash.slice(1) || 'top');
  });

  if ('IntersectionObserver' in window && navSections.length > 0) {
    const visibleSections = new Map();
    const sectionObserver = new window.IntersectionObserver((entries) => {
      entries.forEach((entry) => visibleSections.set(entry.target.id, entry));
      const current = [...visibleSections.values()]
        .filter((entry) => entry.isIntersecting)
        .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
      if (current) setActiveNav(current.target.id);
    }, { rootMargin: '-28% 0px -58% 0px', threshold: 0 });
    navSections.forEach((section) => sectionObserver.observe(section));
  }

  window.addEventListener('resize', moveNavIndicator);

  languageToggle?.addEventListener('click', () => {
    setLanguage(root.dataset.lang === 'en' ? 'zh' : 'en');
  });

  menu?.addEventListener('click', () => {
    const open = nav?.classList.toggle('open') || false;
    menu.setAttribute('aria-expanded', String(open));
  });
  nav?.querySelectorAll('a').forEach((link) => link.addEventListener('click', () => {
    nav.classList.remove('open');
    menu?.setAttribute('aria-expanded', 'false');
  }));

  const tabs = [...document.querySelectorAll('.code-tab')];
  const panels = [...document.querySelectorAll('.code-panel')];
  tabs.forEach((tab) => tab.addEventListener('click', () => {
    const selected = tab.dataset.tab;
    tabs.forEach((item) => {
      const active = item === tab;
      item.classList.toggle('active', active);
      item.setAttribute('aria-selected', String(active));
    });
    panels.forEach((panel) => {
      const active = panel.dataset.panel === selected;
      panel.classList.toggle('active', active);
      panel.setAttribute('aria-hidden', String(!active));
    });
    if (copyButton) copyButton.dataset.copyTarget = selected === 'convert' ? 'api-code' : `api-code-${selected}`;
  }));

  copyButton?.addEventListener('click', async () => {
    const targetId = copyButton.dataset.copyTarget;
    const target = targetId ? document.getElementById(targetId) : null;
    if (!target) return;
    try {
      await navigator.clipboard.writeText(target.textContent);
      copyButton.dataset.copyState = 'done';
      setCopyLabel('done');
      window.setTimeout(() => {
        copyButton.dataset.copyState = 'idle';
        setCopyLabel('idle');
      }, 1400);
    } catch {
      copyButton.dataset.copyState = 'failed';
      setCopyLabel('failed');
    }
  });

  const filterOperations = (value) => {
    const query = value.trim().toLowerCase();
    let visible = 0;
    cards.forEach((card) => {
      const match = !query || (card.dataset.search || '').toLowerCase().includes(query);
      card.hidden = !match;
      if (match) visible += 1;
    });
    if (empty) empty.hidden = visible !== 0;
  };

  operationFilter?.addEventListener('input', (event) => filterOperations(event.target.value));
})();
