const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (s) => document.querySelector(s);
const DAY = 86400;
const KIND_LABEL = {
  node: "node_modules",
  rust: "Rust target",
  web: "Build cache",
  pods: "Pods",
  python: "Python venv",
  gradle: "Gradle",
  flutter: "Flutter",
};

const S = {
  tab: "cleanup",
  home: "",
  targets: [],
  cats: {}, // id -> scan summary
  selCats: new Set(),
  projects: null,
  selProj: new Set(),
  kind: "all",
  root: null,
  scanning: false,
  busy: false,
  disk: { total: 0, free: 0 },
  storage: null, // { sizes: {id: bytes}, done, fullAccess }
};

// ---------- helpers ----------

function el(tag, props = {}, ...kids) {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") n.className = v;
    else if (k === "text") n.textContent = v;
    else if (k.startsWith("on")) n.addEventListener(k.slice(2), v);
    else n.setAttribute(k, v);
  }
  n.append(...kids.filter((k) => k != null));
  return n;
}

function fmt(b) {
  if (b < 1000) return `${b} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let i = -1;
  do { b /= 1000; i++; } while (b >= 1000 && i < units.length - 1);
  return `${b < 10 ? b.toFixed(1) : Math.round(b)} ${units[i]}`;
}

function ago(secs) {
  if (!secs) return "";
  const d = Math.floor((Date.now() / 1000 - secs) / DAY);
  if (d < 1) return "today";
  if (d < 2) return "yesterday";
  if (d < 30) return `${d} days ago`;
  if (d < 365) return `${Math.floor(d / 30)} mo ago`;
  const y = Math.floor(d / 365);
  return `${y} yr${y > 1 ? "s" : ""} ago`;
}

const tilde = (p) => (S.home && p.startsWith(S.home) ? "~" + p.slice(S.home.length) : p);
const base = (p) => p.split("/").filter(Boolean).pop() || p;

function checkbox(checked, disabled, onChange) {
  const cb = el("input", { type: "checkbox", class: "check" });
  cb.checked = checked;
  cb.disabled = disabled;
  cb.addEventListener("change", () => onChange(cb.checked));
  return cb;
}

let toastTimer;
function toast(msg) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("show"), 3200);
}

// ---------- disk ----------

async function refreshDisk() {
  const d = await invoke("disk_info");
  S.home = d.home;
  S.disk = d;
  const used = d.total - d.free;
  const pct = d.total ? (used / d.total) * 100 : 0;
  $("#diskText").innerHTML = `<b>${fmt(d.free)}</b> available of ${fmt(d.total)}`;
  const bar = $("#diskUsed");
  bar.style.width = `${pct}%`;
  bar.classList.toggle("warn", pct > 90);
}

// ---------- cleanup tab ----------

const cleanable = (id) => {
  const c = S.cats[id];
  return c && !c.error && c.size > 0;
};

function renderCleanup() {
  const list = $("#cleanupList");
  list.replaceChildren();

  for (const [group, title] of [["system", "Everyday"], ["dev", "Developer"]]) {
    // Developer tools that aren't installed simply don't show up.
    const ts = S.targets.filter((t) => {
      if (t.group !== group) return false;
      const c = S.cats[t.id];
      return c ? c.found : group === "system";
    });
    if (!ts.length) continue;

    const total = ts.reduce((a, t) => a + (S.cats[t.id]?.size || 0), 0);
    list.append(el("div", { class: "group" }, el("span", { text: title }), el("span", { text: total ? fmt(total) : "" })));

    const card = el("div", { class: "card" });
    for (const t of ts) {
      const c = S.cats[t.id];
      let right;
      if (!c) right = el("span", { class: "size" }, el("i", { class: "spin" }));
      else if (c.error === "needs-access")
        right = el("button", {
          class: "link",
          text: "Allow access",
          title: "Sweep needs Full Disk Access to read this folder",
          onclick: (e) => { e.preventDefault(); invoke("open_full_disk_access"); },
        });
      else right = el("span", { class: "size" + (c.size ? "" : " zero"), text: c.size ? fmt(c.size) : "Empty" });

      const ok = cleanable(t.id);
      const cb = checkbox(ok && S.selCats.has(t.id), !ok, (on) => {
        on ? S.selCats.add(t.id) : S.selCats.delete(t.id);
        saveChoice(t.id, on);
        updateFooter();
      });
      card.append(
        el("label", { class: "row" + (c && !ok ? " dim" : "") },
          cb,
          el("div", { class: "info" }, el("div", { class: "name", text: t.name }), el("div", { class: "desc", text: t.desc })),
          right,
        ),
      );
    }
    list.append(card);
  }
}

// The user's own check/uncheck choices survive restarts. Categories they never
// touched keep following the built-in default.
const CHOICES_KEY = "sweep.choices";

function loadChoices() {
  try {
    return JSON.parse(localStorage.getItem(CHOICES_KEY)) || {};
  } catch {
    return {};
  }
}

function saveChoice(id, on) {
  try {
    const c = loadChoices();
    c[id] = on;
    localStorage.setItem(CHOICES_KEY, JSON.stringify(c));
  } catch {}
}

async function scanCaches() {
  S.cats = {};
  renderCleanup();
  updateFooter();
  await invoke("scan_caches");
}

// ---------- projects tab ----------

function visibleProjects() {
  return (S.projects || []).filter((p) => S.kind === "all" || p.kind === S.kind);
}

function renderProjects() {
  $("#rootBtn").textContent = tilde(S.root || S.home) || "~";
  const chips = $("#chips");
  const list = $("#projectList");
  chips.replaceChildren();
  list.replaceChildren();

  if (S.scanning) {
    list.append(el("div", { class: "empty" }, el("i", { class: "spin" }), el("div", { id: "progress", text: "Looking for projects…" })));
    return;
  }
  if (!S.projects) return;

  if (!S.projects.length) {
    list.append(el("div", { class: "empty" }, el("strong", { text: "All clean" }), el("div", { text: "No dependency or build folders found here." })));
    return;
  }

  const kinds = Object.keys(KIND_LABEL).filter((k) => S.projects.some((p) => p.kind === k));
  const chip = (k, label) => {
    const items = k === "all" ? S.projects : S.projects.filter((p) => p.kind === k);
    const size = items.reduce((a, p) => a + p.size, 0);
    return el("button", {
      class: "chip" + (S.kind === k ? " active" : ""),
      text: `${label} · ${fmt(size)}`,
      onclick: () => { S.kind = k; renderProjects(); },
    });
  };
  chips.append(chip("all", "All"), ...kinds.map((k) => chip(k, KIND_LABEL[k])));

  const items = visibleProjects();
  const allOn = items.every((p) => S.selProj.has(p.id));
  list.append(
    el("div", { class: "group" },
      el("span", { text: `${items.length} folder${items.length === 1 ? "" : "s"}` }),
      el("button", {
        class: "link",
        text: allOn ? "Deselect all" : "Select all",
        onclick: () => {
          items.forEach((p) => (allOn ? S.selProj.delete(p.id) : S.selProj.add(p.id)));
          renderProjects();
          updateFooter();
        },
      }),
    ),
  );

  const card = el("div", { class: "card" });
  for (const p of items) {
    const cb = checkbox(S.selProj.has(p.id), false, (on) => {
      on ? S.selProj.add(p.id) : S.selProj.delete(p.id);
      renderProjects();
      updateFooter();
    });
    const meta = [tilde(p.project), ago(p.modified)].filter(Boolean).join(" · ");
    card.append(
      el("label", { class: "row" },
        cb,
        el("div", { class: "info" },
          el("div", { class: "name" }, el("span", { text: base(p.project) }), el("span", { class: "badge", text: KIND_LABEL[p.kind] || p.kind })),
          el("div", { class: "desc", text: meta, title: p.path }),
        ),
        el("button", {
          class: "icon-btn reveal",
          title: "Show in Finder",
          "aria-label": "Show in Finder",
          onclick: (e) => { e.preventDefault(); invoke("reveal", { path: p.path }); },
        }, revealIcon()),
        el("span", { class: "size", text: fmt(p.size) }),
      ),
    );
  }
  list.append(card);
}

function revealIcon() {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("width", "14");
  svg.setAttribute("height", "14");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", "M9 3h4v4M13 3 7.5 8.5M11 9.5V13H3V5h3.5");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.5");
  path.setAttribute("stroke-linecap", "round");
  path.setAttribute("stroke-linejoin", "round");
  svg.append(path);
  return svg;
}

async function scanProjects() {
  if (S.scanning) return;
  S.scanning = true;
  S.selProj.clear();
  S.kind = "all";
  renderProjects();
  updateFooter();
  try {
    S.projects = await invoke("scan_projects", { root: S.root });
  } catch (e) {
    S.projects = [];
    toast(`Scan failed: ${e}`);
  }
  S.scanning = false;
  renderProjects();
  updateFooter();
}

// ---------- storage tab ----------

// Order matches the backend and fixes each category's color.
const STORAGE = [
  { id: "apps", name: "Apps", desc: "Applications folder" },
  { id: "developer", name: "Developer", desc: "Xcode, simulators and tool folders like ~/.npm, ~/.cargo", go: "cleanup" },
  { id: "documents", name: "Documents & files", desc: "Desktop, Documents, Downloads and other home folders", go: "projects" },
  { id: "media", name: "Photos & media", desc: "Pictures, Movies and Music" },
  { id: "icloud", name: "iCloud Drive", desc: "iCloud files downloaded to this Mac" },
  { id: "caches", name: "Caches & logs", desc: "Rebuilt automatically when needed", go: "cleanup" },
  { id: "appdata", name: "App data", desc: "Application Support, Mail, Messages and app containers" },
  { id: "trash", name: "Trash", desc: "Deleted files waiting to be emptied", go: "cleanup" },
].map((c, i) => ({ ...c, color: `var(--c${i + 1})` }));
const SYSTEM = { id: "system", name: "System & other", desc: "macOS, snapshots and data Sweep can't read", color: "var(--c-sys)" };
const GO_LABEL = { cleanup: "Clean up", projects: "Find projects" };

function storageItems() {
  const items = STORAGE.map((c) => ({ ...c, size: S.storage?.sizes[c.id] }));
  if (S.storage?.done) {
    const measured = items.reduce((a, c) => a + (c.size || 0), 0);
    items.push({ ...SYSTEM, size: Math.max(0, S.disk.total - S.disk.free - measured) });
  }
  return items;
}

function renderStorage() {
  const used = S.disk.total - S.disk.free;
  $("#stUsed").textContent = `${fmt(used)} used`;
  $("#stSub").textContent = `of ${fmt(S.disk.total)} · ${fmt(S.disk.free)} available`;

  const items = storageItems();
  const stack = $("#stack");
  stack.replaceChildren(
    ...items.filter((c) => c.size).map((c) => {
      const seg = el("div", { class: "seg-bar", "data-id": c.id });
      seg.style.background = c.color;
      seg.style.flex = `0 0 ${(c.size / S.disk.total) * 100}%`;
      seg.addEventListener("mousemove", (e) => showTip(c, e));
      seg.addEventListener("mouseleave", hideTip);
      return seg;
    }),
  );

  // Measured categories biggest-first, still-measuring ones at the bottom.
  const sorted = [...items].sort((a, b) => (b.size ?? -1) - (a.size ?? -1));
  const card = el("div", { class: "card" });
  for (const c of sorted) {
    const swatch = el("span", { class: "swatch" });
    swatch.style.background = c.color;
    const pending = c.size === undefined;
    const row = el("div", { class: "row", "data-id": c.id },
      swatch,
      el("div", { class: "info" }, el("div", { class: "name", text: c.name }), el("div", { class: "desc", text: c.desc })),
      c.go && c.size ? el("button", { class: "link", text: GO_LABEL[c.go], onclick: () => switchTab(c.go) }) : null,
      pending
        ? el("span", { class: "size" }, el("i", { class: "spin" }))
        : el("span", { class: "size" + (c.size ? "" : " zero"), text: c.size ? fmt(c.size) : "—" }),
      el("span", { class: "pct", text: pending || !c.size ? "" : pctText(c.size) }),
    );
    row.addEventListener("mouseenter", () => highlight(c.id));
    row.addEventListener("mouseleave", () => highlight(null));
    card.append(row);
  }
  $("#storageList").replaceChildren(el("div", { class: "group" }, el("span", { text: "By category" }), el("span", { text: "of disk" })), card);
  $("#stNote").hidden = !(S.storage?.done && !S.storage.fullAccess);
}

function pctText(size) {
  const p = (size / S.disk.total) * 100;
  return p < 1 ? "<1%" : `${Math.round(p)}%`;
}

function highlight(id) {
  $("#stack").classList.toggle("hovering", !!id);
  document.querySelectorAll("#stack .seg-bar, #storageList .row").forEach((n) => {
    const on = n.dataset.id === id;
    n.classList.toggle(n.classList.contains("row") ? "hl" : "on", on);
  });
}

function showTip(c, e) {
  highlight(c.id);
  const tip = $("#tip");
  tip.replaceChildren(el("b", { text: c.name }), ` · ${fmt(c.size)} · ${pctText(c.size)}`);
  tip.hidden = false;
  const x = Math.min(e.clientX + 12, window.innerWidth - tip.offsetWidth - 8);
  tip.style.left = `${x}px`;
  tip.style.top = `${e.clientY + 16}px`;
}

function hideTip() {
  $("#tip").hidden = true;
  highlight(null);
}

async function scanStorage() {
  if (S.storage && !S.storage.done) return;
  S.storage = { sizes: {}, done: false, fullAccess: true };
  await refreshDisk();
  renderStorage();
  try {
    S.storage.fullAccess = await invoke("scan_storage");
  } catch (e) {
    toast(`Couldn't measure storage: ${e}`);
  }
  S.storage.done = true;
  renderStorage();
}

// ---------- footer & cleaning ----------

function selection() {
  if (S.tab === "storage") return { ids: [], bytes: 0 };
  if (S.tab === "cleanup") {
    const ids = [...S.selCats].filter(cleanable);
    return { ids, bytes: ids.reduce((a, id) => a + S.cats[id].size, 0) };
  }
  const chosen = (S.projects || []).filter((p) => S.selProj.has(p.id));
  return { ids: chosen.map((p) => p.id), bytes: chosen.reduce((a, p) => a + p.size, 0) };
}

function updateFooter() {
  if (S.busy) return; // the progress display owns the footer while deleting
  const { ids, bytes } = selection();
  $("#summary").innerHTML = ids.length
    ? `${ids.length} selected · <b>${fmt(bytes)}</b> to free`
    : "Nothing selected";
  $("#cleanBtn").disabled = !ids.length || S.busy || S.scanning;
}

function confirmDialog(title, body) {
  return new Promise((resolve) => {
    const m = $("#modal");
    $("#mTitle").textContent = title;
    $("#mBody").textContent = body;
    m.hidden = false;
    $("#mOk").focus();
    const done = (v) => {
      m.hidden = true;
      $("#mOk").onclick = $("#mCancel").onclick = m.onclick = document.onkeydown = null;
      resolve(v);
    };
    $("#mOk").onclick = () => done(true);
    $("#mCancel").onclick = () => done(false);
    m.onclick = (e) => e.target === m && done(false);
    document.onkeydown = (e) => e.key === "Escape" && done(false);
  });
}

async function cleanSelected() {
  const { ids, bytes } = selection();
  if (!ids.length) return;
  const body =
    S.tab === "cleanup"
      ? "These files are deleted permanently. Apps and tools recreate caches when they need them."
      : "These folders are deleted permanently. Reinstall or rebuild a project (npm install, cargo build…) to get them back.";
  if (!(await confirmDialog(`Free up ${fmt(bytes)}?`, body))) return;

  const tab = S.tab;
  const btn = $("#cleanBtn");
  S.busy = true;
  S.cleanTotal = bytes;
  document.body.classList.add("busy");
  btn.disabled = true;
  btn.textContent = "Cleaning…";
  $("#progress-bar").hidden = false;
  showProgress(0);
  try {
    const r = await invoke("clean", { ids });
    showProgress(bytes);
    toast(r.errors ? `Freed ${fmt(r.freed)} · ${r.errors} items skipped (in use or protected)` : `Freed ${fmt(r.freed)}`);
    if (tab === "cleanup") {
      scanCaches();
    } else {
      const gone = new Set(ids);
      S.projects = S.projects.filter((p) => !gone.has(p.id));
      S.selProj.clear();
      renderProjects();
    }
  } catch (e) {
    toast(`Something went wrong: ${e}`);
  }
  await new Promise((r) => setTimeout(r, 400)); // let the bar visibly reach the end
  S.busy = false;
  document.body.classList.remove("busy");
  $("#progress-bar").hidden = true;
  btn.textContent = "Clean";
  updateFooter();
  refreshDisk();
  if (S.storage?.done) S.storage = null; // re-measure next time the Storage tab opens
}

function showProgress(freed) {
  const total = S.cleanTotal || 1;
  const pct = Math.min(100, (freed / total) * 100);
  $("#progress-bar div").style.width = `${pct}%`;
  $("#summary").innerHTML = `Deleting… <b>${fmt(Math.min(freed, total))}</b> of ${fmt(total)} · ${Math.floor(pct)}%`;
}

// ---------- wiring ----------

function switchTab(tab) {
  S.tab = tab;
  document.querySelectorAll(".seg button").forEach((b) => b.classList.toggle("active", b.dataset.tab === tab));
  document.querySelectorAll(".panel").forEach((p) => p.classList.toggle("active", p.id === tab));
  document.body.dataset.tab = tab;
  if (tab === "projects" && !S.projects && !S.scanning) scanProjects();
  if (tab === "storage" && !S.storage) scanStorage();
  updateFooter();
}

document.querySelectorAll(".seg button").forEach((b) => b.addEventListener("click", () => switchTab(b.dataset.tab)));
$("#cleanBtn").addEventListener("click", cleanSelected);
$("#rescanBtn").addEventListener("click", scanProjects);
$("#rootBtn").addEventListener("click", async () => {
  const dir = await window.__TAURI__.dialog.open({ directory: true, defaultPath: S.root || S.home });
  if (typeof dir === "string") {
    S.root = dir;
    S.projects = null;
    scanProjects();
  }
});
$("#staleBtn").addEventListener("click", () => {
  const cutoff = Date.now() / 1000 - 30 * DAY;
  visibleProjects().forEach((p) => p.modified && p.modified < cutoff && S.selProj.add(p.id));
  renderProjects();
  updateFooter();
});

listen("category", ({ payload }) => {
  S.cats[payload.id] = payload;
  renderCleanup();
  updateFooter();
});
$("#stRescan").addEventListener("click", scanStorage);
$("#stAccess").addEventListener("click", () => invoke("open_full_disk_access"));
listen("storage", ({ payload }) => {
  if (!S.storage) return;
  S.storage.sizes[payload.id] = payload.size;
  renderStorage();
});
listen("clean-progress",({ payload }) => S.busy && showProgress(payload));
listen("scan-progress",({ payload }) => {
  const p = $("#progress");
  if (p) p.textContent = `Looked through ${payload.toLocaleString()} folders…`;
});

(async function init() {
  await refreshDisk();
  S.targets = await invoke("list_targets");
  const choices = loadChoices();
  S.targets.filter((t) => choices[t.id] ?? t.defaultOn).forEach((t) => S.selCats.add(t.id));
  renderCleanup();
  scanCaches();
})();
