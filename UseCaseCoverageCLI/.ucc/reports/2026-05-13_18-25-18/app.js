
function loadData() {
  const el = document.getElementById('report-data');
  return JSON.parse(el.textContent);
}

function renderMetrics(data) {
  const root = document.getElementById('metrics');
  if (!root) return;
  root.innerHTML = `
    <article class="metric">
      <div class="label">Total Features</div>
      <div class="value-row">
        <div class="value">${data.summary.totalFeatures}</div>
      </div>
    </article>
    <article class="metric">
      <div class="label">Total Use Cases</div>
      <div class="value-row">
        <div class="value">${data.summary.totalUseCases}</div>
        <div class="subtitle gray">${data.summary.totalFeatures > 0 ? (data.summary.totalUseCases / data.summary.totalFeatures).toFixed(1) : 0} / feature</div>
      </div>
    </article>
    <article class="metric accent-border">
      <div class="label">Covered Cases</div>
      <div class="value-row accent-value">
        <div class="value">${data.summary.coveredUseCases}</div>
        <div class="subtitle yellow border-box" style="border:1px solid var(--accent); padding:1px 4px; border-radius:2px; font-size:0.6rem;">${data.summary.useCaseCoveragePct.toFixed(0)}%</div>
      </div>
    </article>
    <article class="metric">
      <div class="label">Total Bugs</div>
      <div class="value-row">
        <div class="value">${data.summary.totalBugs}</div>
        <div class="subtitle gray">${data.summary.totalFeatures > 0 ? (data.summary.totalBugs / data.summary.totalFeatures).toFixed(1) : 0} / feature</div>
      </div>
    </article>
    <article class="metric accent-border" style="border-right:0;">
      <div class="label">Covered Bugs</div>
      <div class="value-row accent-value">
        <div class="value">${data.summary.coveredBugs}</div>
        <div class="subtitle yellow border-box" style="border:1px solid var(--accent); padding:1px 4px; border-radius:2px; font-size:0.6rem;">${data.summary.bugCoveragePct.toFixed(0)}%</div>
      </div>
    </article>
  `;
}

let _sortKey = 'title';
let _sortAsc = true;
let _detailFilter = 'all';
let _detailSort = 'priority';
let _detailSearchText = '';
let _detailUCChart = null;
let _detailBugChart = null;
let _dashboardCharts = [];
let _invCharts = [];
let _tagsViewCharts = [];
let _searchIndex = null;

const PRIORITY_MAP = { 'Highest': 5, 'High': 4, 'Medium': 3, 'Low': 2, 'None': 1 };

function navigate(e, hash) {
  if (e.metaKey || e.ctrlKey) {
    window.open(window.location.pathname + hash, '_blank');
  } else {
    window.location.hash = hash;
  }
}

function renderGapsView(data) {
  const topMissingTable = document.getElementById('topMissingTableBody');
  const topGapsTable = document.getElementById('topGapsTableBody');
  const gapsInventoryTable = document.getElementById('gapsInventoryTableBody');
  if (!topMissingTable || !topGapsTable || !gapsInventoryTable) return;

  const gapReasons = {};
  const featureGaps = [];
  const missingCoverage = [];

  data.features.forEach(f => {
    let gapsInFeature = 0;
    const missingTests = f.useCases - f.useCasesCovered;
    const missingBugs = f.bugs - f.bugsCovered;

    f.artifacts.forEach(a => {
      if (a.coverageGapReason) {
        gapsInFeature++;
        const reason = a.coverageGapReason;
        gapReasons[reason] = gapReasons[reason] || { count: 0, features: new Set() };
        gapReasons[reason].count++;
        gapReasons[reason].features.add(f.title);
      }
    });

    if (gapsInFeature > 0) {
      featureGaps.push({ id: f.id, title: f.title, count: gapsInFeature });
    }
    if (missingTests > 0 || missingBugs > 0) {
      missingCoverage.push({ id: f.id, title: f.title, missingTests, missingBugs });
    }
  });

  // Top Missing
  missingCoverage.sort((a, b) => (b.missingTests + b.missingBugs) - (a.missingTests + a.missingBugs));
  topMissingTable.innerHTML = missingCoverage.slice(0, 10).map(m => `
    <tr onclick="navigate(event, '#feature/${m.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${m.title}</td>
      <td style="color:#fcb714; font-weight:bold;">${m.missingTests}</td>
      <td style="color:#ef4444; font-weight:bold;">${m.missingBugs}</td>
    </tr>
  `).join('') || '<tr><td colspan="3" style="text-align:center; color:var(--text-muted);">No missing coverage</td></tr>';

  // Top Gaps
  featureGaps.sort((a, b) => b.count - a.count);
  topGapsTable.innerHTML = featureGaps.slice(0, 10).map(g => `
    <tr onclick="navigate(event, '#feature/${g.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${g.title}</td>
      <td style="font-weight:bold;">${g.count} artifacts</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No gaps declared</td></tr>';

  // Gaps Inventory
  const reasons = Object.entries(gapReasons).map(([reason, d]) => ({
    reason,
    count: d.count,
    features: Array.from(d.features).join(', ')
  })).sort((a, b) => b.count - a.count);

  gapsInventoryTable.innerHTML = reasons.map(r => `
    <tr>
      <td style="color:#fcb714; max-width:300px;">${r.reason}</td>
      <td style="font-weight:bold;">${r.count}</td>
      <td style="font-size:0.75rem; color:var(--text-muted);">${r.features}</td>
    </tr>
  `).join('') || '<tr><td colspan="3" style="text-align:center; color:var(--text-muted);">No gap reasons found</td></tr>';

  if (typeof d3 !== 'undefined') {
    renderGapCloud(reasons);
  }
}

function renderGapCloud(reasons) {
  const container = document.getElementById('gapCloud');
  if (!container) return;
  container.innerHTML = '';

  const width = container.offsetWidth || 600;
  const height = 400;

  const words = reasons.map(r => ({ text: r.reason, size: 10 + (Math.min(r.count, 20) * 10) }));

  const layout = d3.layout.cloud()
    .size([width, height])
    .words(words)
    .padding(5)
    .rotate(() => (~~(Math.random() * 2) * 90))
    .font("Impact")
    .fontSize(d => d.size)
    .on("end", (words) => {
      const svg = d3.select("#gapCloud").append("svg")
        .attr("width", layout.size()[0])
        .attr("height", layout.size()[1])
        .append("g")
        .attr("transform", "translate(" + layout.size()[0] / 2 + "," + layout.size()[1] / 2 + ")");

      const texts = svg.selectAll("text")
        .data(words)
        .enter().append("text")
        .style("font-size", "0px")
        .style("font-family", "Impact")
        .style("fill", () => d3.schemeTableau10[Math.floor(Math.random() * 10)])
        .attr("text-anchor", "middle")
        .attr("transform", d => "translate(" + [d.x, d.y] + ")rotate(" + d.rotate + ")")
        .text(d => d.text);

      texts.transition()
        .duration(1000)
        .style("font-size", d => d.size + "px");
    });

  layout.start();
}

function getSortValue(feature, key) {
  switch (key) {
    case 'title': return feature.title.toLowerCase();
    case 'useCases': return feature.useCases;
    case 'useCasesCovered': return feature.useCasesCovered;
    case 'ucPct': return feature.useCases > 0 ? feature.useCasesCovered / feature.useCases : -1;
    case 'bugs': return feature.bugs;
    case 'bugsCovered': return feature.bugsCovered;
    case 'bugsPct': return feature.bugs > 0 ? feature.bugsCovered / feature.bugs : -1;
    case 'updatedAt': return feature.updatedAt || feature.createdAt;
    default: return '';
  }
}


function renderPlatformIcon(platform) {
  const p = platform.toLowerCase();
  if (p.includes('apple') || p.includes('ios') || p.includes('mac') || p.includes('iphone')) return '🍎';
  if (p.includes('android')) return '🤖';
  if (p.includes('windows')) return '🪟';
  if (p.includes('web') || p.includes('browser')) return '🌐';
  return '📱';
}
function renderPriorityIcon(priority) {
  const p = priority.toLowerCase();
  let color = '#8b9eb0';
  let svg = '';

  if (p === 'highest') {
    color = '#ff4d4d';
    svg = '<path d="M12 19V5M12 5L5 12M12 5L19 12M12 11L5 18M12 11L19 18" stroke-width="2.5" />';
  } else if (p === 'high') {
    color = '#ff8533';
    svg = '<path d="M12 19V5M12 5L5 12M12 5L19 12" stroke-width="2" />';
  } else if (p === 'medium') {
    color = '#fcb714';
    svg = '<path d="M5 12H19" stroke-width="3" />';
  } else if (p === 'low') {
    color = '#3b82f6';
    svg = '<path d="M12 5V19M12 19L5 12M12 19L19 12" stroke-width="2" />';
  } else {
    svg = '<circle cx="12" cy="12" r="4" fill="currentColor" />';
  }

  return `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="${color}" style="vertical-align: middle; margin-right: 4px;">${svg}</svg>`;
}

function renderFeatureTable(data) {
  const tbody = document.getElementById('featureRows');
  if (!tbody) return;

  const sorted = [...data.features].sort((a, b) => {
    const va = getSortValue(a, _sortKey);
    const vb = getSortValue(b, _sortKey);
    if (va < vb) return _sortAsc ? -1 : 1;
    if (va > vb) return _sortAsc ? 1 : -1;
    return 0;
  });

  tbody.innerHTML = sorted
    .map(
      (feature) => `<tr onclick="navigate(event, '#feature/${feature.id}')" style="cursor:pointer;">
        <td style="color:#a5c8ff; max-width:200px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;" title="${feature.title}">${feature.title}</td>
        <td>${feature.useCases}</td>
        <td>${feature.useCasesCovered}</td>
        <td>${feature.useCases > 0 ? ((feature.useCasesCovered / feature.useCases) * 100).toFixed(0) + '%' : '-'}</td>
        <td>${feature.bugs}</td>
        <td>${feature.bugsCovered}</td>
        <td>${feature.bugs > 0 ? ((feature.bugsCovered / feature.bugs) * 100).toFixed(0) + '%' : '-'}</td>
        <td>${feature.updatedAt ? feature.updatedAt : feature.createdAt}</td>
      </tr>`
    )
    .join('');

  // Update header sort indicators
  document.querySelectorAll('th.sortable').forEach((th) => {
    th.classList.remove('sort-asc', 'sort-desc');
    if (th.dataset.sort === _sortKey) {
      th.classList.add(_sortAsc ? 'sort-asc' : 'sort-desc');
    }
  });
}

function renderFeatureDetail(data, featureId) {
  const container = document.getElementById('featureDetailView');
  const feature = data.features.find(f => f.id === featureId);
  if (!feature) {
    container.innerHTML = '<h1>Feature not found</h1>';
    return;
  }

  let artifacts = [...feature.artifacts];

  // Search
  if (_detailSearchText) {
    const s = _detailSearchText.toLowerCase();
    artifacts = artifacts.filter(a => 
      a.title.toLowerCase().includes(s) || 
      (a.steps || []).some(step => step.toLowerCase().includes(s)) ||
      (a.expected || []).some(exp => exp.toLowerCase().includes(s)) ||
      (a.platforms || []).some(p => p.toLowerCase().includes(s))
    );
  }

  // Filtering
  if (_detailFilter === 'covered') artifacts = artifacts.filter(a => a.isCovered);
  if (_detailFilter === 'missing') artifacts = artifacts.filter(a => !a.isCovered);
  if (_detailFilter === 'critical') artifacts = artifacts.filter(a => !a.isCovered && (a.priority === 'High' || a.priority === 'Highest'));

  // Sorting
  artifacts.sort((a, b) => {
    let va, vb;
    switch (_detailSort) {
      case 'priority':
        va = PRIORITY_MAP[a.priority] || 0;
        vb = PRIORITY_MAP[b.priority] || 0;
        return vb - va; // Default High to Low
      case 'createdAt':
        va = a.createdAt || '';
        vb = b.createdAt || '';
        break;
      case 'updatedAt':
        va = a.updatedAt || a.createdAt || '';
        vb = b.updatedAt || b.createdAt || '';
        break;
      case 'status':
        va = a.isCovered ? 1 : 0;
        vb = b.isCovered ? 1 : 0;
        break;
      default: return 0;
    }
    if (va < vb) return 1;
    if (va > vb) return -1;
    return 0;
  });

  container.innerHTML = `
    <div class="detail-header">
      <button class="back-btn" onclick="navigate(event, '')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>
        Back to Dashboard
      </button>
      <h1 class="detail-title">${feature.title}</h1>
      <div class="detail-meta">
        <span>ID: <strong>${feature.id}</strong></span>
        <span>Created: <strong>${feature.createdAt}</strong></span>
        ${feature.updatedAt ? `<span>Updated: <strong>${feature.updatedAt}</strong></span>` : ''}
      </div>
      <div class="detail-desc">${feature.description || 'No description provided.'}</div>
    </div>

    <section class="charts-row" style="margin-bottom: 2rem; grid-template-columns: 1fr 1fr;">
      <article class="card">
        <div class="card-header">
           <div>
             <h2>Use Cases Coverage</h2>
             <span class="subtitle">${feature.useCasesCovered} / ${feature.useCases} (${feature.useCases > 0 ? ((feature.useCasesCovered/feature.useCases)*100).toFixed(0) : 0}%) Covered</span>
           </div>
        </div>
        <div class="chart-container" style="height: 200px;"><canvas id="detailUCChart"></canvas></div>
      </article>
      <article class="card">
        <div class="card-header">
           <div>
             <h2>Bugs Coverage</h2>
             <span class="subtitle">${feature.bugsCovered} / ${feature.bugs} (${feature.bugs > 0 ? ((feature.bugsCovered/feature.bugs)*100).toFixed(0) : 0}%) Covered</span>
           </div>
        </div>
        <div class="chart-container" style="height: 200px;"><canvas id="detailBugChart"></canvas></div>
      </article>
    </section>

    <div class="detail-controls card" style="display:grid; grid-template-columns: 1fr auto; grid-template-rows: auto auto; padding:1.25rem; gap:1rem; margin-bottom: 2rem; border-radius:12px; background:var(--bg-card); border:1px solid var(--border);">
       <!-- Column 1: Search & Results -->
       <div style="grid-column: 1; display:flex; align-items:center; background:rgba(255,255,255,0.01); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.75rem;"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
         <input type="text" id="detailSearch" value="${_detailSearchText}" placeholder="Search artifacts by title, steps, or content..." 
                oninput="_detailSearchText=this.value; renderFeatureDetail(window._lastData, '${featureId}')"
                style="background:transparent; border:none; color:#fff; padding:1rem 0; width:100%; outline:none; font-size:0.9rem;">
       </div>
       <!-- TOP RIGHT: Results -->
       <div style="grid-column: 2; display:flex; align-items:center; padding:0 1rem; color:var(--text-muted); font-size:0.8rem; white-space:nowrap; border-bottom:1px solid transparent;">
         <strong>${artifacts.length}</strong> &nbsp;results
       </div>
       <!-- BOTTOM LEFT: Filter -->
       <div style="grid-column: 1; display:flex; align-items:center; background:rgba(255,255,255,0.02); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.5rem;"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"></polygon></svg>
         <span style="font-size:0.75rem; font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.05em; margin-right:0.5rem;">Filter</span>
         <select id="filterSelect" onchange="_detailFilter=this.value; renderFeatureDetail(window._lastData, '${featureId}')" style="background:transparent; color:#fff; border:none; padding:0.75rem 0.5rem; font-size:0.9rem; outline:none; cursor:pointer; flex: 1;">
           <option value="all" ${_detailFilter==='all'?'selected':''}>All</option>
           <option value="covered" ${_detailFilter==='covered'?'selected':''}>Covered</option>
           <option value="missing" ${_detailFilter==='missing'?'selected':''}>Missing</option>
           <option value="critical" ${_detailFilter==='critical'?'selected':''}>Critical</option>
         </select>
       </div>
       <!-- BOTTOM RIGHT: Sort -->
       <div style="grid-column: 2; display:flex; align-items:center; background:rgba(255,255,255,0.02); border:1px solid var(--border); border-radius:8px; padding:0 1rem;">
         <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="margin-right:0.5rem;"><path d="M3 12h18M3 6h18M3 18h18"></path></svg>
         <span style="font-size:0.75rem; font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.05em; margin-right:0.5rem;">Sort</span>
         <select id="sortSelect" onchange="_detailSort=this.value; renderFeatureDetail(window._lastData, '${featureId}')" style="background:transparent; color:#fff; border:none; padding:0.75rem 0.5rem; font-size:0.9rem; outline:none; cursor:pointer; flex: 1;">
           <option value="priority" ${_detailSort==='priority'?'selected':''}>Priority</option>
           <option value="createdAt" ${_detailSort==='createdAt'?'selected':''}>Created</option>
           <option value="updatedAt" ${_detailSort==='updatedAt'?'selected':''}>Updated</option>
           <option value="status" ${_detailSort==='status'?'selected':''}>Coverage</option>
         </select>
       </div>
    </div>


    <div class="artifact-section">
      <div class="artifact-grid">
        ${artifacts.map(a => {
          const isHigh = a.priority === 'High' || a.priority === 'Highest';
          const isCritical = isHigh && !a.isCovered;
          return `
          <div class="artifact-card ${isCritical ? 'critical-missing' : ''}">
            <div class="artifact-head">
              <div style="display:flex; align-items:center; gap:0.75rem">
                <span class="badge ${a.type.toLowerCase().includes('bug') ? 'badge-bug' : 'badge-usecase'}">${a.type}</span>
                <span class="artifact-title">${a.title}</span>
              </div>
              <div style="display:flex; gap:0.5rem">
                ${isCritical ? '<span class="badge badge-critical">CRITICAL</span>' : ''}
                <span class="badge ${a.isCovered ? 'badge-covered' : 'badge-missing'}">${a.isCovered ? 'Covered' : 'Missing'}</span>
              </div>
            </div>
            <div class="artifact-body">
              <div style="display:flex; gap:1.5rem; margin-bottom: 0.5rem; font-size:0.8rem; color:var(--text-muted); align-items:center;">
                <div style="display:flex; align-items:center;">Priority: ${renderPriorityIcon(a.priority)} <strong style="color:#fff">${a.priority}</strong></div>
                ${a.platforms && a.platforms.length > 0 ? `
                  <div style="display:flex; align-items:center; gap:0.4rem;">
                    <span style="color:var(--text-muted)">Platforms:</span>
                    ${a.platforms.map(p => `<div style="display:inline-flex; align-items:center; margin-right:0.6rem; font-size:0.7rem;">${renderPlatformIcon(p)} <span style="color:#fff; text-transform:uppercase; letter-spacing:0.02em;">${p}</span></div>`).join('')}
                  </div>
                ` : ''}
                ${a.tags && a.tags.length > 0 ? `
                  <div style="display:flex; align-items:center; gap:0.4rem;">
                    <span style="color:var(--text-muted)">Tags:</span>
                    ${a.tags.map(t => `<span class="badge" style="background:rgba(252,183,20,0.1); color:#fcb714; border:1px solid rgba(252,183,20,0.2); font-size:0.65rem; text-transform:uppercase; padding: 2px 6px;">${t}</span>`).join('')}
                  </div>
                ` : ''}
                <div>Created: <strong>${a.createdAt}</strong></div>
                ${a.updatedAt ? `<div>Updated: <strong>${a.updatedAt}</strong></div>` : ''}
              </div>
              ${a.isCovered && a.coverageLocations && a.coverageLocations.length > 0 ? `
                <div style="position: absolute; bottom: 1.25rem; right: 1.25rem; display:flex; flex-wrap:wrap; gap:0.5rem; justify-content: flex-end;">
                  ${a.coverageLocations.map(loc => `
                    <a href="vscode://file/${loc.path}:${loc.line}" class="back-btn" style="margin:0; padding:4px 8px; font-size:0.75rem; text-decoration:none; display:inline-flex; align-items:center; gap:0.4rem; background:rgba(0,122,204,0.1); border-color:rgba(0,122,204,0.3); color:#4fc1ff;">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M23.15 2.58L19.8 1.45c-.32-.11-.66.1-.66.44v5.45c0 .12-.05.23-.14.31L13 13.7l-3.3-3.04c-.16-.14-.4-.14-.56 0L1 17.72c-.12.11-.12.3 0 .41l3.3 3.04c.16.14.4.14.56 0l1.24-1.14 7.22-6.66c.09-.08.14-.19.14-.31V6.26l6.68-6.16c.16-.14.4-.14.56 0l2.45 2.26c.12.11.12.3 0 .41L18.46 9c-.16.15-.16.4 0 .55l4.69 4.31c.12.11.12.3 0 .41l-2.45 2.26c-.16.14-.4.14-.56 0L13 12.23V17.74c0 .34.34.55.66.44l3.35-1.13c.12-.04.23-.04.35 0l2.45.83c.32.11.66-.1.66-.44V3.02c0-.12-.05-.23-.14-.31l-.18-.13z"/></svg>
                      ${loc.path.split('/').pop().split('\\').pop()}:L${loc.line}
                    </a>
                  `).join('')}
                </div>
              ` : ''}
              ${a.steps && a.steps.length ? `
                <div style="font-weight:600; color:var(--text-muted); font-size:0.8rem; margin-top:0.75rem; text-transform:uppercase;">Steps</div>
                <ol class="steps-list">
                  ${a.steps.map(s => `<li>${s}</li>`).join('')}
                </ol>
              ` : ''}
              ${a.expected && a.expected.length ? `
                <div class="expected-section">
                  <div class="expected-title">Expected Result</div>
                  <ul class="steps-list" style="list-style-type: disc">
                    ${a.expected.map(e => `<li>${e}</li>`).join('')}
                  </ul>
                </div>
              ` : ''}
              ${!a.isCovered && a.coverageGapReason ? `
                <div style="margin-top:1rem; padding:0.75rem; background:rgba(252,183,20,0.05); border-left:3px solid var(--accent); border-radius:4px;">
                  <div style="font-weight:600; color:var(--accent); font-size:0.75rem; text-transform:uppercase; margin-bottom:0.25rem;">Coverage Gap Reason</div>
                  <div style="font-size:0.85rem; color:var(--text-blue); font-style:italic;">"${a.coverageGapReason}"</div>
                </div>
              ` : ''}
            </div>
          </div>
        `;
      }).join('')}
        ${artifacts.length === 0 ? '<div class="card" style="padding:2rem; text-align:center; color:var(--text-muted)">No artifacts match the selected criteria.</div>' : ''}
      </div>
    </div>
  `;

  renderDetailCharts(feature);
  
  const searchInput = document.getElementById('detailSearch');
  if (searchInput) {
    searchInput.focus();
    searchInput.setSelectionRange(searchInput.value.length, searchInput.value.length);
  }
}

function renderDetailCharts(feature) {
  if (_detailUCChart) _detailUCChart.destroy();
  if (_detailBugChart) _detailBugChart.destroy();

  const animationConfig = {
    duration: 1200,
    easing: 'easeOutElastic',
    delay: (context) => context.dataIndex * 300
  };

  const ctxUC = document.getElementById('detailUCChart');
  if (ctxUC) {
    _detailUCChart = new Chart(ctxUC, {
      type: 'doughnut',
      data: {
        labels: ['Covered', 'Missing'],
        datasets: [{
          data: [feature.useCasesCovered, feature.useCases - feature.useCasesCovered],
          backgroundColor: ['#10b981', '#242d38'],
          borderWidth: 0,
          cutout: '70%'
        }]
      },
      options: {
        animation: animationConfig,
        plugins: { legend: { display: false } },
        maintainAspectRatio: false
      }
    });
  }

  const ctxBug = document.getElementById('detailBugChart');
  if (ctxBug) {
    _detailBugChart = new Chart(ctxBug, {
      type: 'doughnut',
      data: {
        labels: ['Covered', 'Missing'],
        datasets: [{
          data: [feature.bugsCovered, feature.bugs - feature.bugsCovered],
          backgroundColor: ['#ef4444', '#242d38'],
          borderWidth: 0,
          cutout: '70%'
        }]
      },
      options: {
        animation: animationConfig,
        plugins: { legend: { display: false } },
        maintainAspectRatio: false
      }
    });
  }
}




function handleRouting(data) {
  window.scrollTo(0, 0);
  const hash = window.location.hash || '#dashboard';
  const dashboard = document.getElementById('dashboardView');
  const detail = document.getElementById('featureDetailView');
  const inventory = document.getElementById('inventoryView');
  const tags = document.getElementById('tagsView');
  const gaps = document.getElementById('gapsView');
  
  [dashboard, detail, inventory, tags, gaps].forEach(v => { if(v) v.style.display = 'none'; });
  
  document.querySelectorAll('.nav-item').forEach(link => {
    const href = link.getAttribute('href');
    link.classList.toggle('active', href === hash || (hash === '#dashboard' && href === '#dashboard'));
  });

  if (hash.startsWith('#feature/')) {
    detail.style.display = 'block';
    renderFeatureDetail(data, hash.replace('#feature/', ''));
  } else if (hash === '#inventory') {
    inventory.style.display = 'block';
    renderInventory(data);
  } else if (hash === '#tags') {
    if (tags) tags.style.display = 'block';
    renderTagsView(data);
  } else if (hash === '#gaps') {
    if (gaps) gaps.style.display = 'block';
    renderGapsView(data);
  } else {
    dashboard.style.display = 'block';
    renderCharts(data);
  }
}

function renderTagsView(data) {
  const tagTable = document.getElementById('tagsTableBody');
  const platformTable = document.getElementById('platformsTableBody');
  if (!tagTable || !platformTable) return;

  const tagCounts = {};
  const platformCounts = {};

  data.features.forEach(f => {
    (f.tags || []).forEach(t => tagCounts[t] = (tagCounts[t] || 0) + 1);
    (f.platforms || []).forEach(p => platformCounts[p] = (platformCounts[p] || 0) + 1);
    (f.artifacts || []).forEach(a => {
        (a.platforms || []).forEach(p => platformCounts[p] = (platformCounts[p] || 0) + 1);
        (a.tags || []).forEach(t => tagCounts[t] = (tagCounts[t] || 0) + 1);
    });
  });

  const tags = Object.entries(tagCounts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);
  const platforms = Object.entries(platformCounts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);

  tagTable.innerHTML = tags.map(item => `
    <tr>
      <td style="color:#fcb714;">${item.name}</td>
      <td style="font-weight:bold;">${item.count}</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No tags found</td></tr>';

  platformTable.innerHTML = platforms.map(item => `
    <tr>
      <td style="color:#a5c8ff;">${item.name}</td>
      <td style="font-weight:bold;">${item.count}</td>
    </tr>
  `).join('') || '<tr><td colspan="2" style="text-align:center; color:var(--text-muted);">No platforms found</td></tr>';

  renderTagsViewCharts(tags, platforms);
}

function renderTagsViewCharts(tags, platforms) {
  _tagsViewCharts.forEach(c => c.destroy());
  _tagsViewCharts = [];

  const createChart = (id, items, color) => {
    const ctx = document.getElementById(id);
    if (!ctx) return;
    _tagsViewCharts.push(new Chart(ctx, {
      type: 'bar',
      data: {
        labels: items.map(i => i.name),
        datasets: [{
          data: items.map(i => i.count),
          backgroundColor: color,
          borderRadius: 4
        }]
      },
      options: {
        indexAxis: 'y',
        animation: { duration: 1000, easing: 'easeOutQuart' },
        plugins: { legend: { display: false } },
        scales: {
          x: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
          y: { grid: { display: false }, ticks: { color: '#fff' } }
        },
        maintainAspectRatio: false
      }
    }));
  };

  createChart('platformsChart', platforms, '#a5c8ff');
  createChart('tagsChart', tags, '#fcb714');
}

function renderInventory(data) {
  if (!_searchIndex && typeof FlexSearch !== 'undefined') {
    _searchIndex = new FlexSearch.Document({
      document: {
        id: "id",
        index: ["title", "description", "content"],
        store: ["id"]
      },
      tokenize: "forward"
    });
    
    data.features.forEach(f => {
       const content = (f.artifacts || []).map(a => 
         `${a.title} ${(a.steps || []).join(' ')} ${(a.expected || []).join(' ')} ${(a.tags || []).join(' ')}`
       ).join(' ');
       const tagContent = (f.tags || []).join(' ');
       const platformContent = [...(f.platforms || []), ...(f.artifacts || []).flatMap(a => a.platforms || [])].join(' ');
       _searchIndex.add({
          id: f.id,
          title: f.title,
          description: f.description || "",
          content: `${content} ${tagContent} ${platformContent}`
       });
    });
  }

  renderInventoryCharts(data);
  renderInventoryTable(data);
  
  const search = document.getElementById('invSearch');
  if (search) {
    search.oninput = (e) => renderInventoryTable(data, e.target.value);
  }
}

function renderInventoryCharts(data) {
  _invCharts.forEach(c => c.destroy());
  _invCharts = [];
  
  const features = data.features;
  const labels = features.map(f => f.title);
  const colors = features.map((_, i) => `hsl(${(i * 360 / features.length) % 360}, 65%, 50%)`);

  const animationConfig = {
    duration: 200,
    easing: 'easeOutQuart',
    delay: (context) => context.dataIndex * 20
  };

  const createChart = (id, label, values, total) => {
    const ctx = document.getElementById(id);
    if (!ctx) return;
    _invCharts.push(new Chart(ctx, {
      type: 'doughnut',
      data: {
        labels: labels,
        datasets: [{ data: values, backgroundColor: colors, borderWidth: 0 }]
      },
      options: {
        animation: animationConfig,
        plugins: { 
          legend: { 
            display: true, 
            position: 'right', 
            labels: { color: '#8b9eb0', boxWidth: 12, padding: 10, font: { size: 10 } } 
          },
          tooltip: { callbacks: { label: (ctx) => `${ctx.label}: ${ctx.raw} ${label}` } }
        },
        maintainAspectRatio: false,
        cutout: '65%'
      },
      plugins: [{
        id: 'centerText',
        beforeDraw: (chart) => {
          const { ctx, width, height } = chart;
          ctx.save();
          ctx.font = 'bold 1.2rem Roboto';
          ctx.fillStyle = '#fff';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const centerLeft = chart.chartArea.left + (chart.chartArea.right - chart.chartArea.left) / 2;
          ctx.fillText(total, centerLeft, height / 2 - 10);
          ctx.font = '0.7rem Roboto';
          ctx.fillStyle = '#8b9eb0';
          ctx.fillText('TOTAL', centerLeft, height / 2 + 15);
          ctx.restore();
        }
      }]
    }));
  };

  const totalUC = features.reduce((sum, f) => sum + f.useCases, 0);
  const totalBugs = features.reduce((sum, f) => sum + f.bugs, 0);
  const totalCovUC = features.reduce((sum, f) => sum + f.useCasesCovered, 0);
  const totalCovBugs = features.reduce((sum, f) => sum + f.bugsCovered, 0);

  createChart('invUCChart', 'Use Cases', features.map(f => f.useCases), totalUC);
  createChart('invBugChart', 'Bugs', features.map(f => f.bugs), totalBugs);
  createChart('invCoveredUCChart', 'Covered UC', features.map(f => f.useCasesCovered), totalCovUC);
  createChart('invCoveredBugChart', 'Covered Bugs', features.map(f => f.bugsCovered), totalCovBugs);
}

function renderInventoryTable(data, filter = '') {
  const tbody = document.getElementById('invRows');
  if (!tbody) return;

  let matched = data.features;
  if (filter && _searchIndex) {
    const results = _searchIndex.search(filter);
    const ids = new Set();
    results.forEach(r => r.result.forEach(id => ids.add(id)));
    matched = data.features.filter(feat => ids.has(feat.id));
  }

  const countEl = document.getElementById('featCount');
  if (countEl) countEl.textContent = `(${matched.length} features)`;

  tbody.innerHTML = matched.map(feat => `
    <tr onclick="navigate(event, '#feature/${feat.id}')" style="cursor:pointer;">
      <td style="color:#a5c8ff;">${feat.title}</td>
      <td style="color:var(--text-muted); font-size:0.85rem;">${feat.description || '-'}</td>
      <td style="color:var(--text-muted); font-size:0.8rem;">${feat.updatedAt || feat.createdAt}</td>
      <td>
        <div style="font-size:0.8rem;">UC: <strong>${feat.useCasesCovered}/${feat.useCases}</strong></div>
        <div style="font-size:0.8rem;">Bugs: <strong>${feat.bugsCovered}/${feat.bugs}</strong></div>
      </td>
    </tr>
  `).join('');
}

function renderLint(data) {
  const list = document.getElementById('lintList');
  if (!list) return;
  if (data.lintIssues.length === 0) {
    list.innerHTML = `<li class="lint-item ok"><strong>All .ucc files passed lint validation.</strong></li>`;
    return;
  }
  list.innerHTML = data.lintIssues
    .map((issue) => {
      const where = issue.line ? `line ${issue.line}${issue.column ? `, col ${issue.column}` : ''}` : 'unknown location';
      return `<li class="lint-item error">
        <div class="lint-path">${issue.file}</div>
        <div class="lint-msg"><strong>${where}</strong>: ${issue.message}</div>
        ${issue.suggestion ? `<div class="lint-msg">Suggestion: ${issue.suggestion}</div>` : ''}
      </li>`;
    })
    .join('');
}

function renderCharts(data) {
  _dashboardCharts.forEach(c => c.destroy());
  _dashboardCharts = [];

  let delayed = false;
  const animationConfig = {
    onComplete: () => { delayed = true; },
    delay: (context) => {
      let delay = 0;
      if (context.type === 'data' && context.mode === 'default' && !delayed) {
        delay = context.dataIndex * 300 + context.datasetIndex * 100;
      }
      return delay;
    }
  };

  const chartConfig = {
    animation: animationConfig,
    plugins: { legend: { display: false } },
    scales: {
      x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
      y: { display: false, grid: { display: false } }
    },
    maintainAspectRatio: false
  };

  const months = data.growth.months;

  const useCaseCanvas = document.getElementById('useCaseGrowthChart');
  if (useCaseCanvas) {
    _dashboardCharts.push(new Chart(useCaseCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    }));
  }

  const featureCanvas = document.getElementById('featureGrowthChart');
  if (featureCanvas) {
    _dashboardCharts.push(new Chart(featureCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Features', data: data.growth.features, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 }
        ]
      },
      options: chartConfig
    }));
  }

  const bugCanvas = document.getElementById('bugGrowthChart');
  if (bugCanvas) {
    _dashboardCharts.push(new Chart(bugCanvas, {
      type: 'bar',
      data: {
        labels: months,
        datasets: [
          { label: 'Bugs', data: data.growth.bugs, backgroundColor: '#fcb714', barPercentage: 0.6 }
        ]
      },
      options: chartConfig
    }));
  }

  const progressCanvas = document.getElementById('featureCoverageChart');
  if (progressCanvas) {
    _dashboardCharts.push(new Chart(progressCanvas, {
      type: 'line',
      data: {
        labels: months,
        datasets: [
          { label: 'Use Cases', data: data.growth.useCases, borderColor: '#96afc9', borderWidth: 2, pointBackgroundColor: '#96afc9', tension: 0.1 },
          { label: 'Covered UC', data: data.growth.coveredUseCases, borderColor: '#a5c8ff', borderWidth: 2, pointBackgroundColor: '#a5c8ff', tension: 0.1, borderDash: [4,3] },
          { label: 'Bugs', data: data.growth.bugs, borderColor: '#fcb714', borderWidth: 2, pointBackgroundColor: '#fcb714', tension: 0.1 },
          { label: 'Covered Bugs', data: data.growth.coveredBugs, borderColor: '#e5a410', borderWidth: 2, pointBackgroundColor: '#e5a410', tension: 0.1, borderDash: [4,3] },
        ]
      },
      options: {
        animation: animationConfig,
        plugins: { 
          legend: { display: true, position: 'top', align: 'end', labels: { boxWidth: 12, color: '#8b9eb0' } }
        },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b9eb0' } },
          y: { grid: { color: '#242d38' }, ticks: { color: '#8b9eb0' } },
        },
        maintainAspectRatio: false
      }
    }));
  }
}

function bootstrap() {
  const data = loadData();
  window._lastData = data;
  renderMetrics(data);
  renderFeatureTable(data);
  renderLint(data);
  renderCharts(data);

  document.querySelectorAll('th.sortable').forEach((th) => {
    th.addEventListener('click', () => {
      const key = th.dataset.sort;
      if (_sortKey === key) {
        _sortAsc = !_sortAsc;
      } else {
        _sortKey = key;
        _sortAsc = true;
      }
      renderFeatureTable(data);
    });
  });

  window.addEventListener('hashchange', () => handleRouting(data));
  handleRouting(data);
}

void bootstrap();
