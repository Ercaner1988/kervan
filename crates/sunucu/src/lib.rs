//! Kervan HTTP/WebSocket sunucusu ve tarayıcı eklentisi köprüsü.
//!
//! kervan'ın işlevsel bölmesi (altin-kapi sandık sınırı, 2026-10-01): bir işlev tek
//! sandıkta. Bağımlı olunan sandığın modülleri kökte yeniden dışa aktarılır; böylece
//! modüllerin `crate::browser` gibi iç yolları bölmeden önceki gibi çalışır.

pub use kervan_havuz::{pool, router};
pub use kervan_mcp::compressor;
pub use kervan_saglayici::{kota, saglayicilar};
pub use kervan_tarayici::{browser, config};

pub use kervan_eklenti::eklenti;
mod eklenti_uc;
mod rest;
pub mod server;
