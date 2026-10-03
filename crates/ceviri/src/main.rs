//! `kervan-ceviri` CLI Aracı
//!
//! Altın parite karşılaştırma betiklerinin Kervan'ın gerçek Rust çeviri
//! kodunu ayrı bir süreçten doğrudan çağırabilmesini sağlar (FR-5.2-a).

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

use kervan_ceviri::*;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Kullanım: kervan-ceviri <claude|gemini> [girdi.json]");
        process::exit(1);
    }

    let hedef = &args[1];
    let girdi_metni = if args.len() >= 3 {
        match fs::read_to_string(&args[2]) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Dosya okunamadı ({}): {}", args[2], e);
                process::exit(1);
            }
        }
    } else {
        let mut buffer = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buffer) {
            eprintln!("Stdin okunamadı: {}", e);
            process::exit(1);
        }
        buffer
    };

    let openai_istek: OpenAiIstek = match serde_json::from_str(&girdi_metni) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Geçersiz OpenAI JSON isteği: {}", e);
            process::exit(1);
        }
    };

    match hedef.as_str() {
        "claude" => {
            let model = "claude-3-7-sonnet";
            match openai_den_claude_istegi(&openai_istek, model) {
                Ok(claude_istek) => {
                    let cikti = serde_json::to_string_pretty(&claude_istek).unwrap_or_default();
                    println!("{}", cikti);
                }
                Err(hata) => {
                    eprintln!("{}", hata);
                    process::exit(2);
                }
            }
        }
        "gemini" => {
            let model = "gemini-2.5-flash";
            match openai_den_gemini_istegi(&openai_istek, model) {
                Ok(gemini_istek) => {
                    let cikti = serde_json::to_string_pretty(&gemini_istek).unwrap_or_default();
                    println!("{}", cikti);
                }
                Err(hata) => {
                    eprintln!("{}", hata);
                    process::exit(2);
                }
            }
        }
        diger => {
            eprintln!("Desteklenmeyen hedef: {}", diger);
            process::exit(1);
        }
    }
}
