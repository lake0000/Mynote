const state = {
  notes: [],
  categories: [],
  activeNote: null,
  settings: {},
  activeCategory: "",
  trashMode: false,
  search: "",
  saveTimer: null,
  dirty: false,
};

const $ = (selector) => document.querySelector(selector);

const els = {
  noteList: $("#noteList"),
  categoryTabs: $("#categoryTabs"),
  searchInput: $("#searchInput"),
  titleInput: $("#titleInput"),
  editor: $("#editor"),
  saveStatus: $("#saveStatus"),
  backupStatus: $("#backupStatus"),
  categorySelect: $("#categorySelect"),
  pinBtn: $("#pinBtn"),
  deleteBtn: $("#deleteBtn"),
  restoreNoteBtn: $("#restoreNoteBtn"),
  fontSizeSelect: $("#fontSizeSelect"),
  themeSelect: $("#themeSelect"),
};

function tauriInvoke() {
  return window.__TAURI__?.core?.invoke || null;
}

async function api(path, options = {}) {
  const invoke = tauriInvoke();
  if (invoke) {
    return tauriApi(invoke, path, options);
  }
  const response = await fetch(path, {
    headers: { "Content-Type": "application/json", ...(options.headers || {}) },
    ...options,
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(payload.error || "请求失败");
  }
  return payload;
}

async function requestBody(options) {
  if (!options.body) return {};
  if (typeof options.body === "string") return JSON.parse(options.body);
  return options.body;
}

async function tauriApi(invoke, path, options = {}) {
  const method = options.method || "GET";
  const url = new URL(path, "http://mynote.local");
  const body = await requestBody(options);
  const noteMatch = url.pathname.match(/^\/api\/notes\/([^/]+)(?:\/(open|delete|restore|quick-save))?$/);

  if (method === "GET" && url.pathname === "/api/bootstrap") {
    return await invoke("bootstrap");
  }
  if (method === "GET" && url.pathname === "/api/notes") {
    return {
      notes: await invoke("list_notes", {
        deleted: url.searchParams.get("deleted") === "1",
        search: url.searchParams.get("search") || null,
        categoryId: url.searchParams.get("category_id") || null,
      }),
    };
  }
  if (method === "POST" && url.pathname === "/api/notes") {
    return {
      note: await invoke("create_note", {
        title: body.title || "未命名日记",
        categoryId: body.category_id || "diary",
      }),
    };
  }
  if (noteMatch) {
    const id = decodeURIComponent(noteMatch[1]);
    const action = noteMatch[2];
    if (method === "GET" && !action) return { note: await invoke("get_note", { id }) };
    if ((method === "PUT" && !action) || (method === "POST" && action === "quick-save")) {
      return { note: await invoke("update_note", { id, data: body }) };
    }
    if (method === "POST" && action === "open") return { note: await invoke("open_note", { id }) };
    if (method === "POST" && action === "delete") {
      await invoke("delete_note", { id });
      return { ok: true };
    }
    if (method === "POST" && action === "restore") return { note: await invoke("restore_note", { id }) };
  }
  if (method === "POST" && url.pathname === "/api/settings") {
    return { settings: await invoke("save_settings", { patch: stringifyValues(body) }) };
  }
  if (method === "POST" && url.pathname === "/api/backup") {
    const backup = await invoke("make_backup");
    const backups = await invoke("list_backups");
    return { backup, backups };
  }
  if (method === "GET" && url.pathname === "/api/backups") {
    return { backups: await invoke("list_backups") };
  }
  if (method === "POST" && url.pathname === "/api/restore") {
    await invoke("restore_backup", { filename: body.filename });
    return { ok: true };
  }
  throw new Error(`Unsupported Tauri API call: ${method} ${path}`);
}

function stringifyValues(value) {
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, String(item)]));
}

function setStatus(text) {
  els.saveStatus.textContent = text;
}

function setBackupStatus(text) {
  els.backupStatus.textContent = text;
}

function categoryName(id) {
  return state.categories.find((item) => item.id === id)?.name || "未分类";
}

function applySettings() {
  const fontSize = state.settings.font_size || "18";
  const theme = state.settings.theme || "rose";
  document.documentElement.style.setProperty("--editor-font-size", `${fontSize}px`);
  document.body.dataset.theme = theme;
  els.fontSizeSelect.value = fontSize;
  els.themeSelect.value = theme;
}

function renderCategories() {
  els.categoryTabs.innerHTML = "";
  const all = document.createElement("button");
  all.textContent = "全部";
  all.className = !state.trashMode && !state.activeCategory ? "active" : "";
  all.addEventListener("click", () => {
    state.trashMode = false;
    state.activeCategory = "";
    loadNotes();
  });
  els.categoryTabs.appendChild(all);

  state.categories.forEach((category) => {
    const button = document.createElement("button");
    button.textContent = category.name;
    button.className = !state.trashMode && state.activeCategory === category.id ? "active" : "";
    button.addEventListener("click", () => {
      state.trashMode = false;
      state.activeCategory = category.id;
      loadNotes();
    });
    els.categoryTabs.appendChild(button);
  });
}

function renderCategorySelect() {
  els.categorySelect.innerHTML = "";
  state.categories.forEach((category) => {
    const option = document.createElement("option");
    option.value = category.id;
    option.textContent = category.name;
    els.categorySelect.appendChild(option);
  });
}

function renderNotes() {
  els.noteList.innerHTML = "";
  if (!state.notes.length) {
    const empty = document.createElement("div");
    empty.className = "note-card";
    empty.innerHTML = `<span class="note-title">没有找到笔记</span><span class="note-meta">换个关键词或新建一篇</span>`;
    els.noteList.appendChild(empty);
    return;
  }

  state.notes.forEach((note) => {
    const button = document.createElement("button");
    button.className = `note-card ${state.activeNote?.id === note.id ? "active" : ""}`;
    button.innerHTML = `
      <span class="note-title">${escapeHtml(note.created_display)} ${escapeHtml(note.title || "未命名日记")}</span>
      <span class="note-meta">
        <span>${escapeHtml(categoryName(note.category_id))}</span>
        <span>${note.is_pinned ? "置顶" : "更新 " + escapeHtml((note.updated_at || "").slice(5, 16).replace("T", " "))}</span>
      </span>
    `;
    button.addEventListener("click", () => openNote(note.id, note.is_deleted));
    els.noteList.appendChild(button);
  });
}

function renderActiveNote() {
  const note = state.activeNote;
  if (!note) return;
  els.titleInput.value = note.title || "";
  els.editor.innerHTML = note.content || "";
  els.categorySelect.value = note.category_id || "diary";
  els.pinBtn.classList.toggle("active", Boolean(note.is_pinned));
  els.pinBtn.textContent = note.is_pinned ? "已置顶" : "置顶";
  els.deleteBtn.classList.toggle("hidden", Boolean(note.is_deleted));
  els.restoreNoteBtn.classList.toggle("hidden", !note.is_deleted);
  els.editor.focus();
  state.dirty = false;
  setStatus("已载入");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function markDirty() {
  if (!state.activeNote || state.activeNote.is_deleted) return;
  state.activeNote.title = els.titleInput.value;
  state.activeNote.content = els.editor.innerHTML;
  state.activeNote.category_id = els.categorySelect.value;
  state.dirty = true;
  setStatus("正在编辑");
  clearTimeout(state.saveTimer);
  state.saveTimer = setTimeout(saveNow, 800);
}

async function saveNow() {
  if (!state.activeNote || !state.dirty || state.activeNote.is_deleted) return;
  clearTimeout(state.saveTimer);
  setStatus("保存中");
  const payload = {
    title: els.titleInput.value,
    content: els.editor.innerHTML,
    category_id: els.categorySelect.value,
    is_pinned: state.activeNote.is_pinned,
  };
  const result = await api(`/api/notes/${encodeURIComponent(state.activeNote.id)}`, {
    method: "PUT",
    body: JSON.stringify(payload),
  });
  state.activeNote = result.note;
  state.dirty = false;
  await loadNotes();
  setStatus("已保存");
}

async function loadNotes(keepActive = true) {
  const params = new URLSearchParams();
  if (state.trashMode) params.set("deleted", "1");
  if (state.search) params.set("search", state.search);
  if (state.activeCategory && !state.trashMode) params.set("category_id", state.activeCategory);
  const result = await api(`/api/notes?${params.toString()}`);
  state.notes = result.notes;
  renderCategories();
  renderNotes();
  if (!keepActive && state.activeNote) {
    renderActiveNote();
  }
}

async function openNote(noteId) {
  await saveNow();
  const result = await api(`/api/notes/${encodeURIComponent(noteId)}/open`, { method: "POST" });
  state.activeNote = result.note;
  renderActiveNote();
  renderNotes();
}

async function createNote() {
  await saveNow();
  const result = await api("/api/notes", {
    method: "POST",
    body: JSON.stringify({ title: "未命名日记", category_id: state.activeCategory || "diary" }),
  });
  state.trashMode = false;
  state.activeNote = result.note;
  await loadNotes();
  renderActiveNote();
}

async function deleteActiveNote() {
  if (!state.activeNote || state.activeNote.is_deleted) return;
  await saveNow();
  await api(`/api/notes/${encodeURIComponent(state.activeNote.id)}/delete`, { method: "POST" });
  setStatus("已移入回收站");
  state.activeNote = null;
  await loadNotes();
  if (state.notes[0]) {
    await openNote(state.notes[0].id);
  } else {
    await createNote();
  }
}

async function restoreActiveNote() {
  if (!state.activeNote) return;
  const result = await api(`/api/notes/${encodeURIComponent(state.activeNote.id)}/restore`, { method: "POST" });
  state.trashMode = false;
  state.activeNote = result.note;
  await loadNotes();
  renderActiveNote();
}

async function togglePin() {
  if (!state.activeNote || state.activeNote.is_deleted) return;
  state.activeNote.is_pinned = !state.activeNote.is_pinned;
  state.dirty = true;
  await saveNow();
}

async function updateSettings(patch) {
  const result = await api("/api/settings", {
    method: "POST",
    body: JSON.stringify(patch),
  });
  state.settings = result.settings;
  applySettings();
}

function bindToolbar() {
  document.querySelectorAll(".toolbar button").forEach((button) => {
    button.addEventListener("click", () => {
      els.editor.focus();
      const command = button.dataset.command;
      const value = button.dataset.value || null;
      document.execCommand(command, false, value);
      markDirty();
    });
  });
}

async function makeBackup() {
  await saveNow();
  const result = await api("/api/backup", { method: "POST" });
  setBackupStatus(`已备份：${result.backup.filename}`);
}

async function restoreLatestBackup() {
  await saveNow();
  const { backups } = await api("/api/backups");
  if (!backups.length) {
    setBackupStatus("暂无备份可恢复");
    return;
  }
  const latest = backups[0].filename;
  if (!window.confirm(`确定从 ${latest} 恢复？当前数据库会被替换。`)) return;
  await api("/api/restore", {
    method: "POST",
    body: JSON.stringify({ filename: latest }),
  });
  setBackupStatus(`已恢复：${latest}`);
  await bootstrap();
}

async function bootstrap() {
  const result = await api("/api/bootstrap");
  state.categories = result.categories;
  state.settings = result.settings;
  state.activeNote = result.active_note;
  state.notes = result.notes;
  if (result.auto_backup) {
    setBackupStatus(`今日自动备份：${result.auto_backup.filename}`);
  }
  applySettings();
  renderCategories();
  renderCategorySelect();
  renderNotes();
  renderActiveNote();
}

function bindEvents() {
  $("#newNoteBtn").addEventListener("click", createNote);
  $("#trashBtn").addEventListener("click", async () => {
    await saveNow();
    state.trashMode = true;
    state.activeCategory = "";
    await loadNotes();
    if (state.notes[0]) {
      const result = await api(`/api/notes/${encodeURIComponent(state.notes[0].id)}`);
      state.activeNote = result.note;
      renderActiveNote();
    }
  });
  $("#backupBtn").addEventListener("click", makeBackup);
  $("#restoreBtn").addEventListener("click", restoreLatestBackup);
  els.deleteBtn.addEventListener("click", deleteActiveNote);
  els.restoreNoteBtn.addEventListener("click", restoreActiveNote);
  els.pinBtn.addEventListener("click", togglePin);
  els.titleInput.addEventListener("input", markDirty);
  els.editor.addEventListener("input", markDirty);
  els.categorySelect.addEventListener("change", markDirty);
  els.fontSizeSelect.addEventListener("change", () => updateSettings({ font_size: els.fontSizeSelect.value }));
  els.themeSelect.addEventListener("change", () => updateSettings({ theme: els.themeSelect.value }));
  els.searchInput.addEventListener("input", () => {
    state.search = els.searchInput.value.trim();
    clearTimeout(state.searchTimer);
    state.searchTimer = setTimeout(() => loadNotes(), 250);
  });

  window.addEventListener("beforeunload", () => {
    if (!state.activeNote || !state.dirty || state.activeNote.is_deleted) return;
    if (tauriInvoke()) {
      saveNow();
      return;
    }
    const payload = JSON.stringify({
      title: els.titleInput.value,
      content: els.editor.innerHTML,
      category_id: els.categorySelect.value,
      is_pinned: state.activeNote.is_pinned,
    });
    navigator.sendBeacon(`/api/notes/${encodeURIComponent(state.activeNote.id)}/quick-save`, new Blob([payload], { type: "application/json" }));
  });
}

bindToolbar();
bindEvents();
bootstrap().catch((error) => {
  setStatus("启动失败");
  setBackupStatus(error.message);
});
