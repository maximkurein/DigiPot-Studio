//! CLI sanity probe of the models: `cargo run -p digipot-core --example probe -- [code]`
use digipot_core::Engine;
use serde_json::json;

fn main() {
    let code: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(128.0);
    let mut e = Engine::new(None);
    let s = e.handle("goto", json!({ "target": "code", "value": code })).expect("goto");
    for m in s["derived"]["metrics"].as_array().unwrap() {
        println!("{:<18} {:>14} {}", m["label"].as_str().unwrap(), m["value"], m["unit"].as_str().unwrap());
    }
    for i in s["derived"]["issues"].as_array().unwrap() {
        println!("[{}] {}", i["level"].as_str().unwrap(), i["text"].as_str().unwrap());
    }
    println!("мультиметр (симулятор): {}", s["meter"]);
    let r = e.handle("read_all", json!({})).expect("read_all");
    println!("чтение по 3-проводной схеме: {}", r["readback"]["words"]);
}
