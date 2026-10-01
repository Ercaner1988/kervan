//! Kervan'in govdesi kutuphane olarak.
//!
//! NEDEN VAR: modullerin tek girisi `main.rs` idi, yani hicbir kod ikili dosyanin
//! disindan cagrilamiyordu — ne olcum araci ne baska bir tuketici. Kutuphane
//! hedefi eklemek davranisi degistirmez (`main.rs` ayni modulleri buradan
//! kullaniyor), ama `benches/` altindaki olcumlerin gercek kodu cagirmasini
//! saglar: olculen sey kopyasi degil kendisi olur.

// İşlevsel bölme (2026-10-01): modüller kendi sandıklarında; cephe aynı adları
// dışa aktarır, `main.rs` ve `benches/` değişmez.
pub use kervan_akis::{audio_video, session_pool, stream_recovery, translator};
#[cfg(feature = "gui")]
pub use kervan_gui::{gui, gui_modeller};
pub use kervan_havuz::{pool, router};
pub use kervan_mcp::{compressor, mcp_server, task_router};
pub use kervan_saglayici::{kota, saglayicilar};
pub use kervan_sunucu::{eklenti, server};
pub use kervan_tarayici::{browser, config, kurulum, surec, tara};
