//! Kervan tarayıcı katmanı: WebDriver oturumu, süreç, tarama, kurulum ve yapılandırma.
//!
//! kervan'ın işlevsel bölmesi (altin-kapi sandık sınırı, 2026-10-01): bir işlev tek
//! sandıkta. Bağımlı olunan sandığın modülleri kökte yeniden dışa aktarılır; böylece
//! modüllerin `crate::browser` gibi iç yolları bölmeden önceki gibi çalışır.

pub mod browser;
pub use kervan_config as config;
pub mod kurulum;
pub mod tara;
