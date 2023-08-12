const { invoke } = window.__TAURI__.tauri;
const { emit, listen } = window.__TAURI__.event;

let sfEl;
let stEl;
let hideSUlink = false;

let pid = "";
let pidEl;
async function jsget_pid() {
    pid = await invoke("get_pantry_id");
    pidEl.value = pid;
}

let lau;
let ldu;
function lau2clip() { navigator.clipboard.writeText(lau); }
function ldu2clip() { navigator.clipboard.writeText(ldu); }

async function send_pid() {
    console.log("Setting PID")
    pid = pidEl.value;
    invoke("set_pantry_id", { pantryid: pid });
    pid_button_visibility();
    show_links();
}

function show_links() {
    if (ldu.length > 0) {
        document.getElementById("live-outer").style.visibility = "visible";
        lau = "https://apps.untan.gl/live/?url=" + ldu;
        document.getElementById("lulink").href = ldu;
        if (!hideSUlink) {
            document.getElementById("lalink").href = lau;
            document.getElementById("su_link").style.display = "inline-block";
        } else {
            document.getElementById("su_link").style.display = "none";
        }
    } else {
        document.getElementById("live-outer").style.visibility = "hidden";
        lau = "";
    }
}

function pid_button_visibility() {
    const pidinput = document.getElementById("pantry-id");
    const pidbut = document.getElementById("pid-button");
    if (pidinput.value == pid) {
        pidbut.style.visibility = "hidden";
    } else {
        pidbut.style.visibility = "visible";
    }
}

window.addEventListener("DOMContentLoaded", () => {
    sfEl = document.querySelector("#scout-file");
    stEl = document.querySelector("#scout-file-status");
    pidEl = document.querySelector("#pantry-id");
    document.querySelector("#selbut").addEventListener("click", (e) => {
        emit("select_file");
    });
    const pidinput = document.getElementById("pantry-id");
    pidinput.addEventListener("input", function() {
        pid_button_visibility();
    });
    emit("dom_loaded");
    jsget_pid();
});

listen("hide_su_link", (event) => { hideSUlink = true; });

listen("set_scout_file", (event) => {
    sfEl.textContent = event.payload.message;
    show_links();
});

listen("scout_file_status", (event) => {
    //console.log("got scout_file_status: " + event.payload.message);
    if (event.payload.message === "ok") {
        stEl.innerHTML =  "<i style=\"color:green;\" class=\"fa-regular fa-circle-check\"></i>"
    } else if (event.payload.message === "uploading") {
        stEl.innerHTML =  "<i style=\"color:black;\" class=\"fa-solid fa-spinner fa-spin\"></i>"
    } else {
        stEl.innerHTML =  "<i style=\"color:red;\" class=\"fa-regular fa-circle-xmark\"></i> (" + event.payload.message + ")";
    }
});

listen("set_live_data_url", (event) => {
    ldu = event.payload.message;
    show_links();
});

listen("set_b64", (event) => {
    document.getElementById("b64").checked = (event.payload.message == "true" | event.payload.message == true);
});

