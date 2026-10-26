//! Olum sinamasi: Kervan zorla oldurulunce actigi tarayici da oluyor mu?
//!
//! BU SINAMA NEDEN VAR: supurme (`artiklari_supur`) yalnizca artiklari temizler;
//! asil koruma, surecin kendisinin bir job object icinde olmasidir. O koruma
//! bozulursa supurme her acilista bir onceki kosunun copunu toplamak zorunda
//! kalir — yani hata sessizce geri gelir. Sinama, hatanin geri gelmesini yakalar.
//!
//! Nasil: `sahip` yardimcisi gercek `agaci_bagla()`'yi kurar ve bir cocuk acar.
//! Sonra `sahip` ZORLA oldurulur (TerminateProcess — hicbir temizlik kodu
//! calismaz). Cocuk yasiyorsa job object korumasi yok demektir.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[test]
fn sahibi_zorla_olunce_cocuk_da_olur() {
    let profil = std::env::temp_dir()
        .join("kervan-olum-sinama")
        .join("a@b.com");
    let _ = std::fs::create_dir_all(&profil);

    let mut sahip = Command::new(env!("CARGO_BIN_EXE_sahip"))
        .arg(profil.display().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sahip baslatilamadi");

    // Yardimci cocuk pid'ini ilan edene kadar bekle.
    let mut satir = String::new();
    BufReader::new(sahip.stdout.take().expect("stdout"))
        .read_line(&mut satir)
        .expect("cocuk pid okunamadi");
    let pid: u32 = satir
        .trim()
        .strip_prefix("COCUK_PID=")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("beklenmeyen satir: {satir:?}"));

    // Cocuk gercekten canli mi? (Yoksa sinama bos bir seyi ispatlardi.)
    assert!(canli(pid), "cocuk baslamamis: {pid}");

    // Sahibi ZORLA oldur: temizlik kodu calismaz, yalnizca isletim sistemi devrede.
    let durum = Command::new("taskkill")
        .args(["/PID", &sahip.id().to_string(), "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("taskkill");
    assert!(durum.success(), "sahip oldurulemedi");
    let _ = sahip.wait();

    let baslangic = Instant::now();
    while baslangic.elapsed() < Duration::from_secs(10) {
        if !canli(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // Temizlik: sinama kirmizi donerse artik surec birakmayalim.
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    panic!("cocuk {pid} 10 sn icinde olmedi — job object korumasi calismiyor");
}

/// Surec yasiyor mu? (pid yeniden kullanilmis olabilir; bu sinamada pencere cok
/// kisa, kabul edilen risk.)
#[cfg(windows)]
fn canli(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}
