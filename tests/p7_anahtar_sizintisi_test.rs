//! FR-6.2 / FR-4.1-d: API anahtarı istemci yanıtına, stdout/stderr'e ve veri dizinindeki
//! hiçbir dosyaya sızmaz. GERÇEK `kervan --serve` ikilisi geçici bir veri diziniyle başlatılır
//! (LOCALAPPDATA dahil her yol geçici dizine yönlenir; kullanıcı verisine dokunulmaz). Sahte
//! yukarı akış anahtarı hata gövdesinde geri yansıtır.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Gelen isteğin Authorization başlığını 401 hata gövdesinde geri yansıtan sahte yukarı akış.
fn yansitan_yukari() -> u16 {
    let dinleyici = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = dinleyici.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for akis in dinleyici.incoming() {
            let Ok(mut s) = akis else { break };
            let mut tampon = [0u8; 8192];
            let n = s.read(&mut tampon).unwrap_or(0);
            let istek = String::from_utf8_lossy(&tampon[..n]).into_owned();
            let yetki = istek
                .lines()
                .find_map(|l| {
                    l.strip_prefix("Authorization: ")
                        .or_else(|| l.strip_prefix("authorization: "))
                })
                .unwrap_or("")
                .to_string();
            let govde = format!(
                r#"{{"error":{{"message":"Incorrect API key provided: {yetki}. Header was Authorization: {yetki}"}}}}"#
            );
            let cevap = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                govde.len(),
                govde
            );
            let _ = s.write_all(cevap.as_bytes());
        }
    });
    port
}

fn bos_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn http_post(port: u16, yol: &str, govde: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    write!(
        s,
        "POST {yol} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{govde}",
        govde.len()
    )
    .unwrap();
    let mut yanit = String::new();
    let _ = s.read_to_string(&mut yanit);
    yanit
}

fn tum_dosyalar(kok: &Path, cikti: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(kok).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            tum_dosyalar(&p, cikti)
        } else {
            cikti.push(p)
        }
    }
}

#[test]
fn anahtar_hicbir_cikti_ya_da_dosyaya_sizmaz() {
    let gizli = format!("sk-sizinti-{:032x}", rand_u128());
    let kok = std::env::temp_dir().join(format!("kervan_sizinti_{:x}", rand_u128()));
    std::fs::create_dir_all(&kok).unwrap();
    // Tarayıcı/sürücü yerine var olan boş dosyalar: havuz tembel açılır, bunlar çalıştırılmaz.
    for d in ["tarayici.exe", "geckodriver.exe"] {
        std::fs::write(kok.join(d), b"").unwrap();
    }
    let yukari_port = yansitan_yukari();
    std::fs::write(
        kok.join("kervan.toml"),
        format!(
            "bosta_dakika = 0\n\n[rest.sizinti]\nbicim = \"openai\"\ntaban_url = \"http://127.0.0.1:{yukari_port}\"\nanahtar_env = \"KERVAN_TEST_SIZINTI_ANAHTARI\"\nmodel = \"m\"\n"
        ),
    )
    .unwrap();

    let port = bos_port();
    let mut cocuk = Command::new(env!("CARGO_BIN_EXE_kervan"))
        .args(["--serve", "--headless", "--port", &port.to_string()])
        .current_dir(&kok)
        .env("LOCALAPPDATA", &kok)
        .env("KERVAN_CONFIG", kok.join("kervan.toml"))
        .env("KERVAN_PROFILES", kok.join("profiles"))
        .env("KERVAN_KOTA", kok.join("kota.jsonl"))
        .env("KERVAN_BROWSER", kok.join("tarayici.exe"))
        .env("KERVAN_GECKODRIVER", kok.join("geckodriver.exe"))
        .env("KERVAN_TEST_SIZINTI_ANAHTARI", &gizli)
        .stdout(std::fs::File::create(kok.join("stdout.txt")).unwrap())
        .stderr(std::fs::File::create(kok.join("stderr.txt")).unwrap())
        .stdin(Stdio::null())
        .spawn()
        .expect("kervan ikilisi başlatılamadı");

    let bas = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        if let Ok(Some(durum)) = cocuk.try_wait() {
            let hata = std::fs::read_to_string(kok.join("stderr.txt")).unwrap_or_default();
            panic!("kervan erken çıktı ({durum}): {hata}");
        }
        assert!(
            bas.elapsed() < Duration::from_secs(60),
            "kervan 60 sn içinde dinlemeye başlamadı"
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    let istek = r#"{"model":"kervan/sizinti","messages":[{"role":"user","content":"selam"}]}"#;
    let akisli = r#"{"model":"kervan/sizinti","stream":true,"messages":[{"role":"user","content":"selam"}]}"#;
    let yanitlar = [
        http_post(port, "/v1/chat/completions", istek),
        http_post(port, "/v1/chat/completions", akisli),
    ];
    let _ = cocuk.kill();
    let _ = cocuk.wait();

    // Testin kendisi doğru kuruldu mu: yukarı akış gerçekten çağrıldı ve 401 istemciye yansıdı.
    for y in &yanitlar {
        assert!(
            y.contains("401") || y.to_lowercase().contains("incorrect api key"),
            "yukarı akış hatası istemciye ulaşmadı: {y}"
        );
        assert!(!y.contains(&gizli), "anahtar istemci yanıtına sızdı: {y}");
    }
    let mut dosyalar = Vec::new();
    tum_dosyalar(&kok, &mut dosyalar);
    assert!(dosyalar.iter().any(|p| p.ends_with("stdout.txt")));
    for p in &dosyalar {
        let icerik = String::from_utf8_lossy(&std::fs::read(p).unwrap()).into_owned();
        assert!(
            !icerik.contains(&gizli),
            "anahtar {} dosyasına sızdı",
            p.display()
        );
    }
    let _ = std::fs::remove_dir_all(&kok);
}

/// Bağımlılıksız rastgele: süreç kimliği + saat + yığın adresi karışımı (test benzersizliği için yeterli).
fn rand_u128() -> u128 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    );
    h.write_u32(std::process::id());
    ((h.finish() as u128) << 64)
        | std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish() as u128
}
