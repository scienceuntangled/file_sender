// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use once_cell::sync::{OnceCell, Lazy};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tauri::api::dialog;
use tauri::{CustomMenuItem, Manager, Menu, Submenu};
use tauri_plugin_store::StoreBuilder;

mod scoutfile;
use scoutfile::ScoutFile;

#[cfg(debug_assertions)]
const IS_DEBUG: bool = true;

#[cfg(not(debug_assertions))]
const IS_DEBUG: bool = false;

#[tauri::command]
fn set_pantry_id(pantryid: String) {
    // set the pantry ID (received from the UI) in the backend
    unsafe {
        let mut this = SCOUT_FILE.lock().unwrap();
        this.set_pantry_id(&pantryid);
        if IS_DEBUG { println!("Pantry ID is set in app: {}", this.get_pantry_id()); }
        // save it
        store_save("pantry_id", json!(&pantryid));
        TAURI_APP_HANDLE.get().unwrap().emit_all("set_live_data_url", Payload { message: this.get_live_data_url() }).unwrap();
    }
}

#[tauri::command]
fn get_pantry_id() -> String {
    // UI to request pantry ID
    unsafe {
        let mut this = SCOUT_FILE.lock().unwrap();
        this.get_pantry_id()
    }
}

fn store_save(key: &str, value: serde_json::Value) {
    let mut store = StoreBuilder::new(TAURI_APP_HANDLE.get().unwrap().clone(), "settings.dat".parse().unwrap()).build();
    let _ = store.load();
    let res = store.insert(key.to_string(), value);
    match res {
        Ok(_) => { store.save().unwrap_or_else(|_| { println!("could not write to store") }); },
        Err(e) => { println!("could not insert value into store: {:?}", e); }
    }
}

fn store_get(key: impl AsRef<str>) -> Option<serde_json::Value> {
    let mut store = StoreBuilder::new(TAURI_APP_HANDLE.get().unwrap().clone(), "settings.dat".parse().unwrap()).build();
    let _ = store.load();
    let val = store.get(key);
    if val.is_none() {
        return None;
    } else {
        let val = val.unwrap();
        return Some(val.clone());
    }
}

// store tauri app_handle
static TAURI_APP_HANDLE: OnceCell<tauri::AppHandle> = OnceCell::new();

static mut SCOUT_FILE: Lazy<Arc<Mutex<ScoutFile>>> = Lazy::new(|| { Arc::new(Mutex::new(ScoutFile::new(TAURI_APP_HANDLE.get().unwrap().clone()))) });

#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // keep app handle
            if TAURI_APP_HANDLE.get().is_none() {
                TAURI_APP_HANDLE.set(app.app_handle()).unwrap();
            }
            // initialize from saved data, if available
            let trypid = store_get("pantry_id");
            if !trypid.is_none() {
                let trypid = trypid.unwrap().as_str().unwrap().to_string();
                unsafe {
                    let mut this = SCOUT_FILE.lock().unwrap();
                    this.set_pantry_id(&trypid.clone());
                }
            }
            let b64 = store_get("b64_encoding");
            if !b64.is_none() {
                let b64 = b64.unwrap().as_bool().unwrap();//.to_string();
                unsafe {
                    let mut this = SCOUT_FILE.lock().unwrap();
                    if !b64 {
                        this.set_b64(false);
                    } else {
                        this.set_b64(true);
                    }
                }
            }

            app.listen_global("dom_loaded", |_| {
                unsafe {
                    let mut this = SCOUT_FILE.lock().unwrap();
                    if !this.get_b64() {
                        TAURI_APP_HANDLE.get().unwrap().emit_all("set_b64", Payload { message: "false".into() }).unwrap();
                    } else {
                        TAURI_APP_HANDLE.get().unwrap().emit_all("set_b64", Payload { message: "true".into() }).unwrap();
                    }
                }
                #[cfg(feature="hide-su-link")]
                TAURI_APP_HANDLE.get().unwrap().emit_all("hide_su_link", Payload { message: "true".into() }).unwrap();
            });

            //let id =
            app.listen_global("select_file", |_| {
                let p = dialog::blocking::FileDialogBuilder::new()
                    .add_filter("Scout file", &["dvw", "vsm"])
                    .pick_file();
                if !p.is_none() {
                    // set in ScoutFile
                    {
                        unsafe {
                            let mut this = SCOUT_FILE.lock().unwrap();
                            this.set_file(p.clone().unwrap());
                            TAURI_APP_HANDLE.get().unwrap().emit_all("set_live_data_url", Payload { message: this.get_live_data_url() }).unwrap();
                        }
                    }
                    // send to UI
                    TAURI_APP_HANDLE.get().unwrap().emit_all("set_scout_file", Payload { message: p.clone().unwrap().to_str().unwrap().into(), }).unwrap();
                } else {
                    // user cancelled, leave existing selection (if there is one)
                }
            });
            app.listen_global("b64_false", |_| {
                unsafe {
                    let mut this = SCOUT_FILE.lock().unwrap();
                    this.set_b64(false);
                }
                store_save("b64_encoding", json!(false));
            });
            app.listen_global("b64_true", |_| {
                unsafe {
                    let mut this = SCOUT_FILE.lock().unwrap();
                    this.set_b64(true);
                }
                store_save("b64_encoding", json!(true));
            });
            Ok(())
        })
        .menu(
            Menu::new().add_submenu(Submenu::new(
                "File",
                Menu::new()
                    .add_item(CustomMenuItem::new("close", "Quit").accelerator("cmdOrControl+Q")),
            )),
        )
        .on_menu_event(|event| match event.menu_item_id() {
            "close" => {
                event.window().close().unwrap();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_pantry_id, set_pantry_id])
        .plugin(tauri_plugin_store::Builder::default().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
