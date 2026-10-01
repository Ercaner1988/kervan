//! Kervan akış işlevleri: akış kurtarma, oturum havuzu kayıtları, çeviri, ses/görüntü (arayüze bağlanacak).
//!
//! kervan'ın işlevsel bölmesi (altin-kapi sandık sınırı, 2026-10-01): bir işlev tek
//! sandıkta. Bağımlı olunan sandığın modülleri kökte yeniden dışa aktarılır; böylece
//! modüllerin `crate::browser` gibi iç yolları bölmeden önceki gibi çalışır.

pub mod audio_video;
pub mod session_pool;
pub mod stream_recovery;
pub mod translator;
