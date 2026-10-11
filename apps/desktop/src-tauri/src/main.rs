use tauri::Manager;
use std::process;

fn main() {
    // Check for headless mode
    let args: Vec<String> = process::args().collect();
    if args.contains(&"--headless".to_string()) {
        println!("Agent Harness Desktop (Headless Mode)");
        println!("Running in headless mode...");
        process::exit(0);
    }

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
