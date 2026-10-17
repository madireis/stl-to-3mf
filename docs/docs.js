/**
 * StlTo3mf Documentation Portal — Interactive Controller
 * Real-time search, sticky scrollspy, 1-click snippet copying, mobile sidebar drawer.
 */

(function () {
  'use strict';

  // DOM Elements
  const searchInput = document.getElementById('docsSearch');
  const searchClearBtn = document.getElementById('clearSearch');
  const searchStatus = document.getElementById('searchStatus');
  const mobileMenuBtn = document.getElementById('mobileMenuBtn');
  const sidebar = document.getElementById('docsSidebar');
  const backdrop = document.getElementById('sidebarBackdrop');
  const navLinks = document.querySelectorAll('.docs-sidebar .nav-link');
  const articles = document.querySelectorAll('.docs-article');
  const copyButtons = document.querySelectorAll('.copy-btn');

  // Cache original innerHTML of searchable containers for clean highlight reset
  const searchableSections = [];
  articles.forEach((art) => {
    searchableSections.push({
      el: art,
      id: art.id,
      title: (art.querySelector('h1, h2') ? art.querySelector('h1, h2').textContent : '').trim(),
      originalHtml: art.innerHTML,
      text: art.textContent.toLowerCase()
    });
  });

  // ==========================================================================
  // 1. One-Click Code Snippet Copying
  // ==========================================================================
  copyButtons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      const codeBox = btn.closest('.code-box');
      if (!codeBox) return;
      const codeEl = codeBox.querySelector('code');
      if (!codeEl) return;

      const textToCopy = codeEl.innerText.trim();

      try {
        if (navigator.clipboard && navigator.clipboard.writeText) {
          await navigator.clipboard.writeText(textToCopy);
        } else {
          // Fallback for older environments
          const textArea = document.createElement('textarea');
          textArea.value = textToCopy;
          textArea.style.position = 'fixed';
          textArea.style.opacity = '0';
          document.body.appendChild(textArea);
          textArea.select();
          document.execCommand('copy');
          document.body.removeChild(textArea);
        }

        const origHtml = btn.innerHTML;
        btn.innerHTML = '<span>✓</span><span>Copied!</span>';
        btn.classList.add('copied');

        setTimeout(() => {
          btn.innerHTML = origHtml;
          btn.classList.remove('copied');
        }, 2000);
      } catch (err) {
        console.error('Failed to copy to clipboard:', err);
      }
    });
  });

  // ==========================================================================
  // 2. Real-Time Client-Side Search
  // ==========================================================================
  let searchDebounceTimer = null;

  function escapeRegExp(str) {
    return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  function highlightMatchesInElement(container, query) {
    if (!query) return 0;
    const regex = new RegExp(`(${escapeRegExp(query)})`, 'gi');
    let matchCount = 0;

    // Recursive text node highlighter
    function walkTextNodes(node) {
      if (node.nodeType === Node.TEXT_NODE) {
        if (node.nodeValue.match(regex)) {
          const count = (node.nodeValue.match(regex) || []).length;
          matchCount += count;
          const span = document.createElement('span');
          span.innerHTML = node.nodeValue.replace(
            regex,
            '<mark class="search-highlight">$1</mark>'
          );
          node.parentNode.replaceChild(span, node);
        }
      } else if (
        node.nodeType === Node.ELEMENT_NODE &&
        !['SCRIPT', 'STYLE', 'BUTTON', 'MARK'].includes(node.tagName)
      ) {
        Array.from(node.childNodes).forEach(walkTextNodes);
      }
    }

    walkTextNodes(container);
    return matchCount;
  }

  function performSearch() {
    const query = (searchInput.value || '').trim();

    if (!query || query.length < 2) {
      // Reset view to original state
      searchClearBtn.style.display = 'none';
      if (searchStatus) searchStatus.style.display = 'none';

      searchableSections.forEach((sec) => {
        sec.el.innerHTML = sec.originalHtml;
        sec.el.style.display = '';
      });

      // Re-attach copy buttons after restoring innerHTML
      reattachCopyButtons();
      updateSidebarActiveState();
      return;
    }

    searchClearBtn.style.display = 'block';
    let totalMatches = 0;
    let matchingArticles = 0;

    searchableSections.forEach((sec) => {
      // First reset to clean original HTML
      sec.el.innerHTML = sec.originalHtml;

      if (sec.text.includes(query.toLowerCase())) {
        sec.el.style.display = '';
        matchingArticles++;
        const matches = highlightMatchesInElement(sec.el, query);
        totalMatches += matches;
      } else {
        sec.el.style.display = 'none';
      }
    });

    // Re-attach copy buttons after search DOM updates
    reattachCopyButtons();

    if (searchStatus) {
      searchStatus.style.display = 'block';
      if (matchingArticles > 0) {
        searchStatus.innerHTML = `🔍 Found <strong>${totalMatches}</strong> match${totalMatches === 1 ? '' : 'es'} across <strong>${matchingArticles}</strong> section${matchingArticles === 1 ? '' : 's'}`;
      } else {
        searchStatus.innerHTML = `⚠️ No matches found for "<strong>${escapeHtml(query)}</strong>"`;
      }
    }
  }

  function reattachCopyButtons() {
    const refreshedCopyBtns = document.querySelectorAll('.copy-btn');
    refreshedCopyBtns.forEach((btn) => {
      // Avoid duplicate listeners
      if (btn.dataset.hasCopyListener) return;
      btn.dataset.hasCopyListener = 'true';

      btn.addEventListener('click', async () => {
        const codeBox = btn.closest('.code-box');
        if (!codeBox) return;
        const codeEl = codeBox.querySelector('code');
        if (!codeEl) return;

        const textToCopy = codeEl.innerText.trim();
        try {
          if (navigator.clipboard && navigator.clipboard.writeText) {
            await navigator.clipboard.writeText(textToCopy);
          }
          const origHtml = btn.innerHTML;
          btn.innerHTML = '<span>✓</span><span>Copied!</span>';
          btn.classList.add('copied');
          setTimeout(() => {
            btn.innerHTML = origHtml;
            btn.classList.remove('copied');
          }, 2000);
        } catch (e) {
          console.error(e);
        }
      });
    });
  }

  function escapeHtml(str) {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  searchInput.addEventListener('input', () => {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(performSearch, 80);
  });

  searchClearBtn.addEventListener('click', () => {
    searchInput.value = '';
    performSearch();
    searchInput.focus();
  });

  // Global Keyboard Shortcuts
  window.addEventListener('keydown', (e) => {
    // Focus search on "/" or Cmd/Ctrl + K
    if (
      (e.key === '/' && document.activeElement !== searchInput) ||
      ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'K'))
    ) {
      e.preventDefault();
      searchInput.focus();
      searchInput.select();
    }
    // Clear & blur search on "Escape"
    if (e.key === 'Escape') {
      if (searchInput.value) {
        searchInput.value = '';
        performSearch();
      }
      searchInput.blur();
      closeMobileSidebar();
    }
  });

  // ==========================================================================
  // 3. Sticky Sidebar Navigation ScrollSpy (IntersectionObserver)
  // ==========================================================================
  function updateSidebarActiveState() {
    const scrollPos = window.scrollY + 120;
    let currentId = '';

    articles.forEach((art) => {
      if (art.style.display === 'none') return;
      const top = art.offsetTop;
      const height = art.offsetHeight;
      if (scrollPos >= top && scrollPos < top + height) {
        currentId = art.id;
      }
    });

    if (!currentId && articles.length > 0 && articles[0].style.display !== 'none') {
      currentId = articles[0].id;
    }

    navLinks.forEach((link) => {
      const href = link.getAttribute('href');
      if (href === `#${currentId}`) {
        link.classList.add('active');
      } else {
        link.classList.remove('active');
      }
    });
  }

  window.addEventListener('scroll', updateSidebarActiveState, { passive: true });
  updateSidebarActiveState();

  // Smooth click scroll on nav links
  navLinks.forEach((link) => {
    link.addEventListener('click', (e) => {
      const targetId = link.getAttribute('href');
      if (targetId && targetId.startsWith('#')) {
        const targetEl = document.querySelector(targetId);
        if (targetEl) {
          e.preventDefault();
          closeMobileSidebar();
          targetEl.scrollIntoView({ behavior: 'smooth' });
          history.replaceState(null, '', targetId);
          setTimeout(updateSidebarActiveState, 200);
        }
      }
    });
  });

  // ==========================================================================
  // 4. Mobile Sidebar Drawer Toggle
  // ==========================================================================
  function openMobileSidebar() {
    sidebar.classList.add('open');
    backdrop.classList.add('active');
    document.body.style.overflow = 'hidden';
  }

  function closeMobileSidebar() {
    sidebar.classList.remove('open');
    backdrop.classList.remove('active');
    document.body.style.overflow = '';
  }

  if (mobileMenuBtn) {
    mobileMenuBtn.addEventListener('click', () => {
      if (sidebar.classList.contains('open')) {
        closeMobileSidebar();
      } else {
        openMobileSidebar();
      }
    });
  }

  if (backdrop) {
    backdrop.addEventListener('click', closeMobileSidebar);
  }

  // Initialize button state
  reattachCopyButtons();
})();
