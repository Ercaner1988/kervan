//! Sinama yardimcisi: gercek `agaci_bagla()`'yi kurar, bir cocuk surec acar ve
//! uyur.
//!
//! YALNIZ TEST ICIN. Olum sinamasinin sarti: job object'i kuran surec ZORLA
//! olduruldugunde cocugun da olmesi. Bunu ancak gercek bir ikili ile
//! gosterebiliriz — sinama surecinin kendisi job'a girip intihar edemez.

use std::io::Write;

fn main() {
    kervan_surec::agaci_bagla();

    let profil = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\kervan-sinama\profil".to_string());
    let uyuyan = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("uyuyan.exe")))
        .unwrap_or_default();
    let cocuk = match std::process::Command::new(&uyuyan)
        .arg("-profile")
        .arg(&profil)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("uyuyan baslatilamadi ({}): {e}", uyuyan.display());
            std::process::exit(1);
        }
    };

    println!("COCUK_PID={}", cocuk.id());
    // Sinama pid'i okumak icin ciktiyi bekliyor; tampon bosaltilmazsa kilitlenir.
    if let Err(e) = std::io::stdout().flush() {
        eprintln!("cikti bosaltilamadi: {e}");
    }
    std::mem::forget(cocuk);

    std::thread::sleep(std::time::Duration::from_secs(120));
}
