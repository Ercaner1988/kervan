//! Kervan HTTP/WebSocket sunucusu ve tarayıcı eklentisi köprüsü.
//!
//! kervan'ın işlevsel bölmesi (altin-kapi sandık sınırı, 2026-10-01): bir işlev tek
//! sandıkta. Bağımlı olunan sandığın modülleri kökte yeniden dışa aktarılır; böylece
//! modüllerin `crate::browser` gibi iç yolları bölmeden önceki gibi çalışır.

pub use kervan_havuz::{pool, router};
pub use kervan_mcp::compressor;
pub use kervan_saglayici::{kota, saglayicilar};
pub use kervan_tarayici::{browser, config, kurulum};

pub use kervan_eklenti::eklenti;
/// REST yukarı akış sandığı `yukari` adıyla yeniden dışa aktarılır: arayüzün
/// oturum kasasına (`oturum_anahtari_ekle` / `anahtar_coz`) erişebilmesi için.
/// `kervan-gui` bu sandığa doğrudan bağımlı değil; tek kapı burasıdır.
pub use kervan_yukari as yukari;
mod chat;
mod eklenti_uc;
mod hesap;
mod kasa_uc;
mod messages;
mod olc;
mod olcum;
mod rest;
pub mod server;
