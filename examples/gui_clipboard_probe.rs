//! Synthetic ownership driver for the isolated native GUI plugin acceptance.
use std::io::{BufRead, Write};
fn main() {
    let session = omarchy_t3_client::gui_clipboard::start().expect("actual GUI admission");
    assert!(session.alive());
    println!("admitted");
    std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines() {
        match line.unwrap().as_str() {
            "alive" => {
                println!("{}", session.alive());
                std::io::stdout().flush().unwrap();
            }
            "quit" => break,
            _ => panic!("unknown private fixture command"),
        }
    }
}
