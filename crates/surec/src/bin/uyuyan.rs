//! Sinama yardimcisi: verilen argumanlarla baslar ve uyur.
//!
//! YALNIZ TEST ICIN. Supurme sinamasi, komut satirinda `-profile <dizin>` tasiyan
//! gercek bir surec gerektiriyor; sahte komut satiri kurmanin kullanici alaninda
//! yolu yok (`CreateProcess` cagrisini biz yapmadikca). Bu ikili o rolu oynar:
//! argumanlari yalnizca komut satirinda tasir, islevi uyumaktir.

fn main() {
    let saniye = std::env::var("KERVAN_UYUYAN_SANIYE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120u64);
    std::thread::sleep(std::time::Duration::from_secs(saniye));
}
