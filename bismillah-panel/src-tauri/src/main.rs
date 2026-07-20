// src-tauri/src/main.rs
//
// Backend de la "carcaza". Responsabilidades, y nada más que estas:
//   1. Leer el .env real del disco y devolverlo parseado al frontend.
//   2. Escribir los valores nuevos de vuelta al .env, preservando
//      comentarios, orden y líneas en blanco (solo cambia los valores).
//   3. Prender/apagar el proceso del bot (bismillah_bot.exe) como hijo.
//   4. Guardar/leer un settings.json chico (al lado del .exe de la app)
//      con las rutas al .exe del bot y al .env, elegidas una vez por
//      el usuario con el selector de archivos nativo de Windows.
//
// A propósito, este backend NO hace ninguna llamada de red — ni
// reqwest, ni nada que hable con internet. Solo filesystem + procesos
// locales. Cualquiera que audite este archivo puede confirmarlo.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

// ---------- Estado compartido en memoria mientras la app corre ----------

struct BotProcess(Mutex<Option<Child>>);

#[derive(Serialize, Deserialize, Default, Clone)]
struct AppSettings {
    bot_exe_path: Option<String>,
    env_path: Option<String>,
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("no se pudo resolver el directorio de config de la app");
    fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

// ---------- Comandos invocables desde el frontend (JS) ----------

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> AppSettings {
    let path = settings_path(&app);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = settings_path(&app);
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

/// Lee el .env real. Devuelve tanto el mapa clave→valor (para poblar
/// los campos del formulario) como las líneas crudas originales (para
/// poder reescribirlo después sin destruir comentarios ni orden).
#[tauri::command]
fn read_env_file(path: String) -> Result<HashMap<String, String>, String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("No pude leer {}: {}", path, e))?;
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Ok(map)
}

/// Escribe los valores nuevos de vuelta al .env. Preserva cada línea
/// que no cambió (comentarios, separadores, variables no tocadas) y
/// solo reemplaza el valor de las claves que vinieron en `updates`.
#[tauri::command]
fn write_env_file(path: String, updates: HashMap<String, String>) -> Result<(), String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("No pude leer {}: {}", path, e))?;
    let mut new_lines: Vec<String> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            new_lines.push(line.to_string());
            continue;
        }
        if let Some((key, _old_value)) = trimmed.split_once('=') {
            let key = key.trim().to_string();
            if let Some(new_value) = updates.get(&key) {
                new_lines.push(format!("{}={}", key, new_value));
                seen_keys.insert(key);
                continue;
            }
        }
        new_lines.push(line.to_string());
    }

    // Si alguna clave nueva no existía en el archivo original, se agrega al final.
    for (key, value) in updates.iter() {
        if !seen_keys.contains(key) {
            new_lines.push(format!("{}={}", key, value));
        }
    }

    let mut out = new_lines.join("\n");
    out.push('\n');
    fs::write(&path, out).map_err(|e| format!("No pude escribir {}: {}", path, e))
}

#[tauri::command]
fn bot_status(state: State<BotProcess>) -> bool {
    let mut guard = state.0.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(Some(_status)) => {
                *guard = None; // el proceso ya terminó solo
                false
            }
            Ok(None) => true, // sigue corriendo
            Err(_) => false,
        }
    } else {
        false
    }
}

#[derive(Serialize, Clone)]
struct BotLogLine {
    stream: &'static str, // "stdout" | "stderr"
    line: String,
}

#[tauri::command]
fn start_bot(app: tauri::AppHandle, exe_path: String, state: State<BotProcess>) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();

    // Si ya hay uno corriendo, no arrancamos dos veces.
    if let Some(child) = guard.as_mut() {
        if let Ok(None) = child.try_wait() {
            return Err("El bot ya está corriendo".to_string());
        }
    }

    let path = PathBuf::from(&exe_path);
    let working_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));

    let mut child = Command::new(&path)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("No pude iniciar el bot: {}", e))?;

    // Cada línea que el bot escribe por stdout/stderr se reenvía al
    // frontend como evento "bot-log", para que la consola en vivo del
    // panel se vea igual que la terminal real.
    if let Some(stdout) = child.stdout.take() {
        let app_handle = app.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = app_handle.emit("bot-log", BotLogLine { stream: "stdout", line });
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app_handle = app.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = app_handle.emit("bot-log", BotLogLine { stream: "stderr", line });
            }
        });
    }

    *guard = Some(child);
    Ok(())
}

#[tauri::command]
fn stop_bot(state: State<BotProcess>) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();
    if let Some(mut child) = guard.take() {
        child.kill().map_err(|e| format!("No pude detener el bot: {}", e))?;
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command]
fn restart_bot(app: tauri::AppHandle, exe_path: String, state: State<BotProcess>) -> Result<(), String> {
    stop_bot(state.clone())?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    start_bot(app, exe_path, state)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(BotProcess(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            read_env_file,
            write_env_file,
            bot_status,
            start_bot,
            stop_bot,
            restart_bot,
        ])
        .run(tauri::generate_context!())
        .expect("error corriendo la app de Tauri");
}
