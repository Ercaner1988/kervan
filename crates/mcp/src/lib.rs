//! Kervan MCP sunucusu, görev sınıflandırıcı ve istem sıkıştırıcı.
//!
//! kervan'ın işlevsel bölmesi (altin-kapi sandık sınırı, 2026-10-01): bir işlev tek
//! sandıkta. Bağımlı olunan sandığın modülleri kökte yeniden dışa aktarılır; böylece
//! modüllerin `crate::browser` gibi iç yolları bölmeden önceki gibi çalışır.

pub mod compressor;
pub mod mcp_server;
pub mod task_router;
