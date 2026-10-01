//! Kervan sağlayıcı defteri ve kota öğrenicisi.
//!
//! kervan'ın işlevsel bölmesi (altin-kapi sandık sınırı, 2026-10-01): bir işlev tek
//! sandıkta. Bağımlı olunan sandığın modülleri kökte yeniden dışa aktarılır; böylece
//! modüllerin `crate::browser` gibi iç yolları bölmeden önceki gibi çalışır.

pub use kervan_tarayici::{browser, config};

pub mod kota;
pub mod saglayicilar;
