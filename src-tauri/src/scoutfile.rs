use base64::{Engine as _, engine::general_purpose};
use chrono::prelude::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::{PathBuf};
use std::sync::{Arc, Mutex};
use std::{fs, thread};
use std::io::Read;
use std::time::Duration;
use tauri::Manager;
use tokio::task::block_in_place;
use uuid::Uuid;

#[cfg(debug_assertions)]
const IS_DEBUG: bool = true;

#[cfg(not(debug_assertions))]
const IS_DEBUG: bool = false;

// nicer printing of os errors
fn nice_err(e: impl Error) -> String {
    let re = Regex::new(r"\(os.*").unwrap();
    re.replace(&format!("{}", e), "").into_owned()
}

const REFRACTORY_PERIOD: u64 = 3;

#[derive(Clone, Serialize)]
struct Payload {
    message: String,
}

// use a debounced file watcher, so that multiple events issued at almost the same time will be collapsed to a single event
use notify_debouncer_mini::new_debouncer;
fn watch2(path: PathBuf, sf: Arc<Mutex<ScoutFileInner>>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(std::time::Duration::from_millis(250), None, tx).unwrap();
    thread::spawn(move || {
        debouncer.watcher().watch(path.clone().as_path(), notify::RecursiveMode::NonRecursive).unwrap();
        block_in_place(|| loop {
            for result in &rx {
                match result {
                    Ok(events) => events.iter().for_each(|e| {
                        if IS_DEBUG { println!("watch event: {:?}", e); }
                        let this = Arc::clone(&sf);
                        sf_set_modified(this);
                    }),
                    Err(error) => { if IS_DEBUG { println!("Error {error:?}") }},
                }
            }
        });
    });
}

// struct to hold info about the scout file
struct ScoutFileInner {
    path: PathBuf,
    busy: bool,
    modified: bool,
    pantry_id: String,
    b64: bool,
    app: tauri::AppHandle,
}

// the data will be passed between threads, so wrap it in an Arc/Mutex
pub struct ScoutFile { inner: Arc<Mutex<ScoutFileInner>> }

// check if we need to send the file
// this function will be called periodically
fn sf_check_send(sf: Arc<Mutex<ScoutFileInner>>) {
    let temp = Arc::clone(&sf);
    let local_self = temp.lock().unwrap();

    if local_self.path.as_path().to_str().unwrap().len() < 1 {
        // file path has not been set
        local_self.app.emit_all("scout_file_status", Payload { message: "na".into() }).unwrap();
        return
    } else if local_self.modified && !local_self.busy {
        if IS_DEBUG { println!("Needs sending"); }
        local_self.app.emit_all("scout_file_status", Payload { message: "uploading".into() }).unwrap();
        drop(local_self);
        sf_send(sf);
    } else if local_self.busy {
        if IS_DEBUG { println!("Needs sending but busy"); }
    } else {
        if IS_DEBUG { println!("Does not need sending"); }
        // send OK if file exists
        match local_self.path.try_exists() {
            Ok(_) => { local_self.app.emit_all("scout_file_status", Payload { message: "ok".into() }).unwrap(); },
            Err(_) => { local_self.app.emit_all("scout_file_status", Payload { message: "file does not exist".into() }).unwrap(); }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PostData {
    filename: String,
    data: String,
    last_modified: String,
}

// send the file
fn sf_send(sf: Arc<Mutex<ScoutFileInner>>) {
    if IS_DEBUG { println!("  <in sf_send>"); }
    let temp = Arc::clone(&sf);
    let mut local_self = temp.lock().unwrap();
    if local_self.pantry_id.len() < 1 {
        local_self.app.emit_all("scout_file_status", Payload { message: "Pantry ID needed".into() }).unwrap();
        return;
    }
    if IS_DEBUG { println!("Sending: {}", local_self.path.display()); }
    let send_id = Uuid::new_v4().simple().to_string(); // just for debugging purposes
    if IS_DEBUG { println!("  {} Busy 1: {}", &send_id[..5], local_self.busy); }
    {
        local_self.busy = true;
    }
    if IS_DEBUG { println!("  {} Busy 2: {}", &send_id[..5], local_self.busy); }
    let temp2 = Arc::clone(&sf);
    let send_id2 = send_id.clone();
    thread::spawn(move || {
        {
            if IS_DEBUG { println!("  {} <in inner, 1>", &send_id2[..5]); }
            let mut local_self2 = temp2.lock().unwrap();
            local_self2.app.emit_all("scout_file_status", Payload { message: "uploading".into() }).unwrap();
            let thisp = local_self2.path.clone();
            let cont2: String;
            if local_self2.b64 {
                // read as bytes, and 64-encode that
                let f = fs::File::open(thisp.clone().as_path());
                let mut fdata = vec![];
                match f {
                    Err(e) => {
                        // could not read file, maybe it doesn't exist or we don't have permissions
                        if IS_DEBUG { println!("  {} <file could not be read>", &send_id2[..5]); }
                        // pass meaningful message to front end
                        local_self2.app.emit_all("scout_file_status", Payload { message: nice_err(e) }).unwrap();
                        drop(local_self2); // CHECK are we better not to unlock in this kind of situation?
                        // now a refractory period during which we cannot try again
                        thread::sleep(Duration::from_secs(REFRACTORY_PERIOD));
                        if IS_DEBUG { println!("  {} <in inner, 2>", &send_id2[..5]); }
                        local_self2 = temp2.lock().unwrap();
                        local_self2.busy = false;
                        if IS_DEBUG { println!("  {} Busy inner: {}", &send_id2[..5], local_self2.busy); }
                        local_self2.busy = false;
                        return
                    },
                    Ok(_) => {
                        f.unwrap().read_to_end(&mut fdata).unwrap();
                        cont2 = general_purpose::STANDARD.encode(&fdata);
                    }
                }
            } else {
                let cont = fs::read_to_string(thisp.clone().as_path());
                match cont {
                    Err(e) => {
                        // could not read file, maybe it doesn't exist or we don't have permissions
                        if IS_DEBUG { println!("  {} <file could not be read>", &send_id2[..5]); }
                        // pass meaningful message to front end
                        local_self2.app.emit_all("scout_file_status", Payload { message: nice_err(e) }).unwrap();
                        drop(local_self2); // CHECK are we better not to unlock in this kind of situation?
                        // now a refractory period during which we cannot try again
                        thread::sleep(Duration::from_secs(REFRACTORY_PERIOD));
                        if IS_DEBUG { println!("  {} <in inner, 2>", &send_id2[..5]); }
                        local_self2 = temp2.lock().unwrap();
                        local_self2.busy = false;
                        if IS_DEBUG { println!("  {} Busy inner: {}", &send_id2[..5], local_self2.busy); }
                        local_self2.busy = false;
                        return
                    },
                    Ok(_) => {
                        cont2 = cont.unwrap();
                    }
                }
            }
            // do the actual POST operation
            let desturl = make_live_data_url(local_self2.pantry_id.clone(), get_basket_name(local_self2.path.clone()));
            if IS_DEBUG { println!("Will be posting to: {}", desturl.clone()); }
            drop(local_self2); // release lock while sending
            let fname = thisp.as_path().file_name().unwrap().to_str().unwrap();
            let post_data = PostData { filename: fname.into(), data: cont2, last_modified: format!("{:?}", Utc::now()) };

            let client = reqwest::blocking::Client::new()
                .post(desturl)
                .header("Content-Type", "application/json")
                .json(&post_data).send().unwrap();

            local_self2 = temp2.lock().unwrap();
            if client.status().is_success() {
                local_self2.modified = false;
                if IS_DEBUG { println!("success!"); }
            } else if client.status().is_server_error() {
                if IS_DEBUG { println!("server error! {:?}", client); }
                local_self2.app.emit_all("scout_file_status", Payload { message: "failed".into() }).unwrap(); // TODO get err msg or code
            } else {
                if IS_DEBUG { println!("Something else happened. Status: {:?}", client.status()); }
                local_self2.app.emit_all("scout_file_status", Payload { message: "failed".into() }).unwrap(); // TODO get err msg or code
            }
            drop(local_self2); // release lock while sending
            // now a refractory period during which we cannot send again
            thread::sleep(Duration::from_secs(REFRACTORY_PERIOD));
            if IS_DEBUG { println!("  {} <in inner, 2>", &send_id2[..5]); }
            local_self2 = temp2.lock().unwrap();
            local_self2.busy = false;
            if IS_DEBUG { println!("  {} Busy inner: {}", &send_id2[..5], local_self2.busy); }
            // once the timer expires we need to trigger check_send
            drop(local_self2);
            sf_check_send(temp2);
        }
    });
    if IS_DEBUG { println!("  {} Busy 3: {}", &send_id[..5], local_self.busy); }
}

// indicate that the file has been modified, and call sf_check_send
fn sf_set_modified(sf: Arc<Mutex<ScoutFileInner>>) {
    let temp = Arc::clone(&sf);
    let mut local_self = temp.lock().unwrap();
    local_self.modified = true;
    drop(local_self);
    sf_check_send(temp);
}

fn make_live_data_url(pantry_id: String, basket_name: String) -> String {
    if pantry_id.len() > 0 && basket_name.len() > 0 {
        format!("https://getpantry.cloud/apiv1/pantry/{}/basket/{}", pantry_id, basket_name)
    } else {
        "".into()
    }
}

use urlencoding;
fn get_basket_name(p: PathBuf) -> String {
    let ok = p.try_exists();
    match ok {
        Ok(v) => {
            if v {
                let b: String = p.file_name().unwrap().to_str().unwrap().into();
                return urlencoding::encode(&b).into();
            } else {
                return "".into();
            }
        },
        Err(_) => { return "".into(); }
    }
}

impl ScoutFile {
    // construct empty ScoutFile object
    pub fn new(app: tauri::AppHandle) -> ScoutFile {
        ScoutFile { inner: Arc::new(Mutex::new(
            ScoutFileInner { path: PathBuf::new(),
                             busy: false,
                             modified: false,
                             pantry_id: "".to_string(),
                             b64: true,
                             app: app
            }
        ))}
    }

    pub fn get_live_data_url(&mut self) -> String {
        let this = self.inner.lock().unwrap();
        let bn = get_basket_name(this.path.clone());
        if IS_DEBUG { println!("Basket name: {:?}", bn); }
        make_live_data_url(this.pantry_id.clone(), bn)
    }

    pub fn get_b64(&mut self) -> bool {
        self.inner.lock().unwrap().b64.clone()
    }
    pub fn set_b64(&mut self, b64: bool) {
        self.inner.lock().unwrap().b64 = b64;
        let this = Arc::clone(&self.inner);
        sf_check_send(this);
    }

    pub fn set_pantry_id(&mut self, pantry_id: &str) {
        self.inner.lock().unwrap().pantry_id = pantry_id.to_string();
        let this = Arc::clone(&self.inner);
        sf_check_send(this);
    }
    pub fn get_pantry_id(&mut self) -> String {
        self.inner.lock().unwrap().pantry_id.clone()
    }

    // assign a file, set a watcher to watch it, and periodically check whether we need to send it
    pub fn set_file(&mut self, path: PathBuf) {
        let local_self = Arc::clone(&self.inner);
        let path2 = path.clone();
        let mut this = local_self.lock().unwrap();
        this.path = path;
        this.modified = true; // set as modified initially, so we force an initial upload
        drop(this);
        // set a watcher on this file
        watch2(path2, local_self);
        let this = Arc::clone(&self.inner);
        thread::spawn(move || {
            block_in_place(|| loop {
                thread::sleep(Duration::from_secs(1));
                let this2 = Arc::clone(&this);
                sf_check_send(this2);
            });
        });
    }

    // not used yet
    //    pub fn status(&self) {
    //      let temp = Arc::clone(&self.inner);
    //      let local_self = temp.lock().unwrap();
    //      println!("{}\n  Busy: {}\n  Modified: {}", local_self.path.display(), local_self.busy, local_self.modified);
    //    }
}
