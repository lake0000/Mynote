#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{Datelike, Local};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct AppPaths {
    data_dir: PathBuf,
    db_path: PathBuf,
    backup_dir: PathBuf,
    attachments_dir: PathBuf,
}

#[derive(Serialize)]
struct Category {
    id: String,
    name: String,
    sort_order: i64,
    created_at: String,
}

#[derive(Serialize, Clone)]
struct Note {
    id: String,
    title: String,
    content: String,
    category_id: Option<String>,
    is_pinned: bool,
    is_deleted: bool,
    created_at: String,
    updated_at: String,
    last_opened_at: Option<String>,
    created_display: String,
}

#[derive(Serialize)]
struct BackupInfo {
    filename: String,
    size: u64,
    modified_at: String,
}

#[derive(Serialize)]
struct Bootstrap {
    active_note: Note,
    notes: Vec<Note>,
    categories: Vec<Category>,
    settings: HashMap<String, String>,
    backups: Vec<BackupInfo>,
    auto_backup: Option<BackupInfo>,
}

#[derive(Deserialize)]
struct NoteInput {
    title: Option<String>,
    content: Option<String>,
    category_id: Option<String>,
    is_pinned: Option<bool>,
}

#[derive(Deserialize)]
struct ListInput {
    deleted: Option<bool>,
    search: Option<String>,
    category_id: Option<String>,
}

const DEFAULT_CATEGORIES: [(&str, &str); 6] = [
    ("diary", "日记"),
    ("quote", "摘抄"),
    ("idea", "想法"),
    ("study", "学习"),
    ("project", "项目"),
    ("mood", "情绪"),
];

fn app_home() -> PathBuf {
    if let Ok(home) = std::env::var("MYNOTE_HOME") {
        return PathBuf::from(home);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn paths() -> AppPaths {
    let data_dir = app_home().join("data");
    AppPaths {
        db_path: data_dir.join("diary.db"),
        backup_dir: data_dir.join("backups"),
        attachments_dir: data_dir.join("attachments"),
        data_dir,
    }
}

fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn today_key() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn note_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("note-{nanos}")
}

fn created_display(value: &str) -> String {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .map(|dt| format!("{}/{}/{}", dt.year(), dt.month(), dt.day()))
        .unwrap_or_else(|_| value.get(0..10).unwrap_or(value).replace('-', "/"))
}

fn sanitize_content(value: &str) -> String {
    let mut out = value.to_string();
    while let Some(start) = out.to_lowercase().find("<script") {
        if let Some(end) = out[start..].to_lowercase().find("</script>") {
            out.replace_range(start..start + end + 9, "");
        } else {
            out.replace_range(start.., "");
        }
    }
    out
}

fn ensure_dirs(p: &AppPaths) -> Result<(), String> {
    fs::create_dir_all(&p.data_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&p.backup_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&p.attachments_dir).map_err(|e| e.to_string())?;
    Ok(())
}

fn connect(p: &AppPaths) -> Result<Connection, String> {
    ensure_dirs(p)?;
    let conn = Connection::open(&p.db_path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn get_setting(conn: &Connection, key: &str, default: &str) -> Result<String, String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?", [key], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())
        .map(|v| v.unwrap_or_else(|| default.to_string()))
}

fn put_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn init_db(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS notes (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          content TEXT NOT NULL,
          category_id TEXT,
          is_pinned INTEGER NOT NULL DEFAULT 0,
          is_deleted INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          last_opened_at TEXT
        );
        CREATE TABLE IF NOT EXISTS settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if count == 0 {
        let ts = now_iso();
        for (idx, (id, name)) in DEFAULT_CATEGORIES.iter().enumerate() {
            conn.execute(
                "INSERT INTO categories (id, name, sort_order, created_at) VALUES (?, ?, ?, ?)",
                params![id, name, idx as i64, ts],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let font_size = get_setting(conn, "font_size", "18")?;
    let theme = get_setting(conn, "theme", "rose")?;
    let sidebar = get_setting(conn, "sidebar_width", "330")?;
    put_setting(conn, "font_size", &font_size)?;
    put_setting(conn, "theme", &theme)?;
    put_setting(conn, "sidebar_width", &sidebar)?;
    Ok(())
}

fn row_to_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    let created_at: String = row.get("created_at")?;
    Ok(Note {
        id: row.get("id")?,
        title: row.get("title")?,
        content: row.get("content")?,
        category_id: row.get("category_id")?,
        is_pinned: row.get::<_, i64>("is_pinned")? != 0,
        is_deleted: row.get::<_, i64>("is_deleted")? != 0,
        created_display: created_display(&created_at),
        created_at,
        updated_at: row.get("updated_at")?,
        last_opened_at: row.get("last_opened_at")?,
    })
}

fn categories_inner(conn: &Connection) -> Result<Vec<Category>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, sort_order, created_at FROM categories ORDER BY sort_order, name")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(Category {
            id: row.get(0)?,
            name: row.get(1)?,
            sort_order: row.get(2)?,
            created_at: row.get(3)?,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn settings_inner(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings").map_err(|e| e.to_string())?;
    let pairs = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(pairs.into_iter().collect())
}

fn get_note_inner(conn: &Connection, id: &str) -> Result<Option<Note>, String> {
    conn.query_row("SELECT * FROM notes WHERE id = ?", [id], row_to_note)
        .optional()
        .map_err(|e| e.to_string())
}

fn mark_opened(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("UPDATE notes SET last_opened_at = ? WHERE id = ?", params![now_iso(), id])
        .map_err(|e| e.to_string())?;
    put_setting(conn, "last_opened_note_id", id)
}

fn create_note_inner(conn: &Connection, title: &str, category_id: &str) -> Result<Note, String> {
    let id = note_id();
    let ts = now_iso();
    conn.execute(
        "INSERT INTO notes
         (id, title, content, category_id, is_pinned, is_deleted, created_at, updated_at, last_opened_at)
         VALUES (?, ?, '', ?, 0, 0, ?, ?, ?)",
        params![id, title, category_id, ts, ts, ts],
    )
    .map_err(|e| e.to_string())?;
    put_setting(conn, "last_opened_note_id", &id)?;
    get_note_inner(conn, &id)?.ok_or_else(|| "note not found after create".to_string())
}

fn open_last_or_today(conn: &Connection) -> Result<Note, String> {
    let last_id = get_setting(conn, "last_opened_note_id", "")?;
    if !last_id.is_empty() {
        if let Some(note) = conn
            .query_row(
                "SELECT * FROM notes WHERE id = ? AND is_deleted = 0",
                [&last_id],
                row_to_note,
            )
            .optional()
            .map_err(|e| e.to_string())?
        {
            mark_opened(conn, &note.id)?;
            return Ok(note);
        }
    }
    let today = today_key();
    if let Some(note) = conn
        .query_row(
            "SELECT * FROM notes WHERE is_deleted = 0 AND category_id = 'diary' AND substr(created_at, 1, 10) = ? ORDER BY created_at ASC LIMIT 1",
            [today],
            row_to_note,
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        mark_opened(conn, &note.id)?;
        return Ok(note);
    }
    create_note_inner(conn, "今天的记录", "diary")
}

fn list_notes_inner(conn: &Connection, input: ListInput) -> Result<Vec<Note>, String> {
    let deleted = if input.deleted.unwrap_or(false) { 1 } else { 0 };
    let search = input.search.unwrap_or_default();
    let category_id = input.category_id.unwrap_or_default();
    let mut sql = "SELECT * FROM notes WHERE is_deleted = ?".to_string();
    let mut values: Vec<String> = vec![deleted.to_string()];
    if !category_id.is_empty() {
        sql.push_str(" AND category_id = ?");
        values.push(category_id);
    }
    if !search.is_empty() {
        sql.push_str(" AND (title LIKE ? OR content LIKE ?)");
        values.push(format!("%{search}%"));
        values.push(format!("%{search}%"));
    }
    sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC");
    let params = rusqlite::params_from_iter(values.iter());
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params, row_to_note)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn backup_info(path: PathBuf) -> Option<BackupInfo> {
    let meta = path.metadata().ok()?;
    let modified = meta.modified().ok()?;
    let dt: chrono::DateTime<Local> = modified.into();
    Some(BackupInfo {
        filename: path.file_name()?.to_string_lossy().to_string(),
        size: meta.len(),
        modified_at: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
    })
}

fn list_backups_inner(p: &AppPaths) -> Result<Vec<BackupInfo>, String> {
    ensure_dirs(p)?;
    let mut items = fs::read_dir(&p.backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok().and_then(|e| backup_info(e.path())))
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(items)
}

fn make_backup_inner(conn: &Connection, p: &AppPaths, label: &str) -> Result<BackupInfo, String> {
    ensure_dirs(p)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;
    let stamp = Local::now().format("%Y-%m-%d-%H%M%S");
    let target = p.backup_dir.join(format!("diary-{stamp}-{label}.db"));
    fs::copy(&p.db_path, &target).map_err(|e| e.to_string())?;
    backup_info(target).ok_or_else(|| "backup metadata unavailable".to_string())
}

fn ensure_daily_backup(conn: &Connection, p: &AppPaths) -> Result<Option<BackupInfo>, String> {
    let today = today_key();
    if get_setting(conn, "last_auto_backup_date", "")? == today {
        return Ok(None);
    }
    let backup = make_backup_inner(conn, p, "auto")?;
    put_setting(conn, "last_auto_backup_date", &today)?;
    Ok(Some(backup))
}

#[tauri::command]
fn bootstrap() -> Result<Bootstrap, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    let active_note = open_last_or_today(&conn)?;
    let auto_backup = ensure_daily_backup(&conn, &p)?;
    Ok(Bootstrap {
        active_note,
        notes: list_notes_inner(&conn, ListInput { deleted: None, search: None, category_id: None })?,
        categories: categories_inner(&conn)?,
        settings: settings_inner(&conn)?,
        backups: list_backups_inner(&p)?,
        auto_backup,
    })
}

#[tauri::command]
fn list_notes(deleted: Option<bool>, search: Option<String>, category_id: Option<String>) -> Result<Vec<Note>, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    list_notes_inner(&conn, ListInput { deleted, search, category_id })
}

#[tauri::command]
fn get_note(id: String) -> Result<Note, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    get_note_inner(&conn, &id)?.ok_or_else(|| "note not found".to_string())
}

#[tauri::command]
fn create_note(title: Option<String>, category_id: Option<String>) -> Result<Note, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    create_note_inner(
        &conn,
        title.as_deref().unwrap_or("未命名日记"),
        category_id.as_deref().unwrap_or("diary"),
    )
}

#[tauri::command]
fn update_note(id: String, data: NoteInput) -> Result<Note, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    let existing = get_note_inner(&conn, &id)?.ok_or_else(|| "note not found".to_string())?;
    let ts = now_iso();
    conn.execute(
        "UPDATE notes SET title = ?, content = ?, category_id = ?, is_pinned = ?, updated_at = ? WHERE id = ?",
        params![
            data.title.unwrap_or(existing.title),
            sanitize_content(&data.content.unwrap_or(existing.content)),
            data.category_id.or(existing.category_id),
            if data.is_pinned.unwrap_or(existing.is_pinned) { 1 } else { 0 },
            ts,
            id
        ],
    )
    .map_err(|e| e.to_string())?;
    get_note_inner(&conn, &id)?.ok_or_else(|| "note not found".to_string())
}

#[tauri::command]
fn open_note(id: String) -> Result<Note, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    mark_opened(&conn, &id)?;
    get_note_inner(&conn, &id)?.ok_or_else(|| "note not found".to_string())
}

#[tauri::command]
fn delete_note(id: String) -> Result<(), String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    conn.execute("UPDATE notes SET is_deleted = 1, updated_at = ? WHERE id = ?", params![now_iso(), id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn restore_note(id: String) -> Result<Note, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    conn.execute("UPDATE notes SET is_deleted = 0, updated_at = ? WHERE id = ?", params![now_iso(), id])
        .map_err(|e| e.to_string())?;
    mark_opened(&conn, &id)?;
    get_note_inner(&conn, &id)?.ok_or_else(|| "note not found".to_string())
}

#[tauri::command]
fn save_settings(patch: HashMap<String, String>) -> Result<HashMap<String, String>, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    for (key, value) in patch {
        if ["font_size", "theme", "sidebar_width", "last_opened_note_id"].contains(&key.as_str()) {
            put_setting(&conn, &key, &value)?;
        }
    }
    settings_inner(&conn)
}

#[tauri::command]
fn make_backup() -> Result<BackupInfo, String> {
    let p = paths();
    let conn = connect(&p)?;
    init_db(&conn)?;
    make_backup_inner(&conn, &p, "manual")
}

#[tauri::command]
fn list_backups() -> Result<Vec<BackupInfo>, String> {
    list_backups_inner(&paths())
}

#[tauri::command]
fn restore_backup(filename: String) -> Result<(), String> {
    let p = paths();
    ensure_dirs(&p)?;
    let name = Path::new(&filename)
        .file_name()
        .ok_or_else(|| "invalid filename".to_string())?
        .to_string_lossy()
        .to_string();
    let source = p.backup_dir.join(name);
    if !source.is_file() {
        return Err("backup not found".to_string());
    }
    fs::copy(source, &p.db_path).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(format!("{}-wal", p.db_path.display()));
    let _ = fs::remove_file(format!("{}-shm", p.db_path.display()));
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            list_notes,
            get_note,
            create_note,
            update_note,
            open_note,
            delete_note,
            restore_note,
            save_settings,
            make_backup,
            list_backups,
            restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mynote");
}
