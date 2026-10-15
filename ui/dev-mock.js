// Fake backend so the UI can be previewed in a normal browser (not loaded inside the app).
(() => {
  const handlers = {};
  const emit = (ev, payload) => (handlers[ev] || []).forEach((h) => h({ payload }));
  const G = 1e9, M = 1e6, home = "/Users/demo", now = Date.now() / 1000;
  const targets = [
    ["caches", "system", "App caches", "Temporary files apps rebuild automatically", true, 4.2 * G],
    ["logs", "system", "Logs", "Diagnostic logs and crash reports", true, 180 * M],
    ["installers", "system", "Installers in Downloads", "Leftover .dmg, .pkg and .xip files", false, 2.1 * G],
    ["trash", "system", "Trash", "Files you've already deleted", false, -1],
    ["npm", "dev", "npm cache", "~/.npm download cache and npx installs", true, 3.4 * G],
    ["yarn", "dev", "Yarn cache", "Classic and Berry package caches", true, 1.2 * G],
    ["derived", "dev", "Xcode DerivedData", "Build products and indexes", true, 18.6 * G],
    ["cocoapods", "dev", "CocoaPods cache", "Downloaded pod specs and sources", true, 640 * M],
    ["homebrew", "dev", "Homebrew cache", "Downloaded bottles and source archives", true, 0],
  ];
  const projects = [
    ["FloatingLyrics", "node", 812 * M, 200], ["AwesomeProject", "node", 1.4 * G, 400],
    ["AwesomeProject/ios", "pods", 920 * M, 400], ["my-product", "node", 640 * M, 3],
    ["my-product", "web", 210 * M, 3], ["crypto", "rust", 2.3 * G, 90], ["AI", "python", 1.1 * G, 40],
  ].map(([p, kind, size, d]) => {
    const project = `${home}/Documents/Coding/${p}`;
    const path = `${project}/${{ node: "node_modules", pods: "Pods", web: ".next", rust: "target", python: ".venv" }[kind]}`;
    return { id: "p:" + path, kind, path, project, size, modified: now - d * 86400 };
  });

  const commands = {
    disk_info: () => ({ total: 494 * G, free: 31 * G, home }),
    list_targets: () => targets.map(([id, group, name, desc, defaultOn]) => ({ id, group, name, desc, defaultOn })),
    scan_caches: () => targets.forEach(([id, , , , , size], i) =>
      setTimeout(() => emit("category", { id, size: Math.max(size, 0), count: 3, found: true, error: size < 0 ? "needs-access" : null }), 150 + i * 120)),
    scan_projects: () => new Promise((r) => {
      let n = 0;
      const t = setInterval(() => emit("scan-progress", (n += 4321)), 150);
      setTimeout(() => { clearInterval(t); r(projects); }, 1200);
    }),
    scan_storage: () => new Promise((r) => {
      const sizes = { apps: 54.8, developer: 55, documents: 66.4, media: 8.1, icloud: 2.3, caches: 4.2, appdata: 110.7, trash: 0 };
      Object.entries(sizes).forEach(([id, gb], i) => setTimeout(() => emit("storage", { id, size: gb * G }), 200 + i * 150));
      setTimeout(() => r(false), 1500);
    }),
    clean: () => new Promise((r) => {
      let freed = 0;
      const t = setInterval(() => emit("clean-progress", (freed += 0.4 * G)), 100);
      setTimeout(() => { clearInterval(t); r({ freed, errors: 0 }); }, 1500);
    }),
  };

  window.__TAURI__ = {
    core: { invoke: async (cmd) => commands[cmd]?.() },
    event: { listen: async (ev, h) => (handlers[ev] = [...(handlers[ev] || []), h]) },
    dialog: { open: async () => null },
  };
})();
