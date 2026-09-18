//! Kervan'in govdesi kutuphane olarak.
//!
//! NEDEN VAR: modullerin tek girisi `main.rs` idi, yani hicbir kod ikili dosyanin
//! disindan cagrilamiyordu — ne olcum araci ne baska bir tuketici. Kutuphane
//! hedefi eklemek davranisi degistirmez (`main.rs` ayni modulleri buradan
//! kullaniyor), ama `benches/` altindaki olcumlerin gercek kodu cagirmasini
//! saglar: olculen sey kopyasi degil kendisi olur.

pub mod browser;
pub mod config;
pub mod kota;
pub mod kurulum;
pub mod pool;
pub mod saglayicilar;
pub mod server;
pub mod surec;
pub mod tara;
