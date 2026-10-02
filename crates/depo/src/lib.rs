//! Kervan Veri Katmanı ve Depo Soyutlaması (`Depo` Trait)
//!
//! OmniRoute veritabanı şeması ve 193 SQL göçünün yönetimi.

pub mod gocler;
pub mod rusqlite_depo;
pub mod sifreleme;

pub use rusqlite_depo::RusqliteDepo;
pub use sifreleme::Sifreleyici;

use anyhow::Result;

/// Kervan Veri Katmanı Genel Trait'i (ADR 0002)
pub trait Depo: Send + Sync {
    /// 193 SQL göçünü sırayla uygular ve uygulanan göç sayısını döner.
    fn gocleri_calistir(&self) -> Result<usize>;

    /// Veritabanındaki toplam aktif tablo sayısını döner (Altın şema: 136).
    fn tablo_sayisi(&self) -> Result<usize>;

    /// SQLite bütünlük kontrolünü (`PRAGMA integrity_check`) koşturur.
    fn saglik_kontrolu(&self) -> Result<bool>;

    /// `VACUUM INTO` kullanarak sıcak veritabanını hedef yola yedekler.
    fn yedekle(&self, hedef_yol: &str) -> Result<()>;

    /// Güvenli anahtar-değer deposundan okuma yapar (otomatik deşifre eder).
    fn deger_getir(&self, tablo: &str, anahtar: &str) -> Result<Option<String>>;

    /// Güvenli anahtar-değer deposuna yazma yapar (otomatik şifreler).
    fn deger_kaydet(&self, tablo: &str, anahtar: &str, deger: &str) -> Result<()>;
}
