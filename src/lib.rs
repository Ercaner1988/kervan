//! Kervan'in govdesi kutuphane olarak.
//!
//! NEDEN VAR: modullerin tek girisi `main.rs` idi, yani hicbir kod ikili dosyanin
//! disindan cagrilamiyordu — ne olcum araci ne baska bir tuketici. Kutuphane
//! hedefi eklemek davranisi degistirmez (`main.rs` ayni modulleri buradan
//! kullaniyor), ama `benches/` altindaki olcumlerin gercek kodu cagirmasini
//! saglar: olculen sey kopyasi degil kendisi olur.

pub mod audio_video;
pub mod browser;
pub mod compressor;
pub mod config;
pub mod eklenti;
pub mod gui;
pub mod gui_modeller;
pub mod kota;
pub mod kurulum;
pub mod pool;
pub mod router;
pub mod saglayicilar;
pub mod server;
pub mod session_pool;
pub mod surec;
pub mod tara;
pub mod mcp_server;
pub mod stream_recovery;
pub mod task_router;
pub mod translator;
