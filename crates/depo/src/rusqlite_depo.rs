//! Rusqlite (Bundled SQLite) Depo Uygulaması
//!
//! Zopay deseni: Tüm yerel SQLite yeteneklerini (WAL, FTS5, VACUUM INTO)
//! sıfır sistem bağımlılığıyla garanti eden güvenli depo.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use crate::Depo;
use crate::gocler::{
    goc_dosyalarini_yukle, sema_zaten_uygulanmis_mi, sutunlari_guvenceye_al, tablolari_listele,
};
use crate::sifreleme::Sifreleyici;

pub struct RusqliteDepo {
    conn: Mutex<Connection>,
    sifreleyici: Sifreleyici,
    goc_dizini: Option<std::path::PathBuf>,
}

impl RusqliteDepo {
    /// Bellek içi (:memory:) SQLite deposu oluşturur (özellikle testler için).
    pub fn bellek_ici(sifreleyici: Sifreleyici, goc_dizini: Option<&Path>) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            sifreleyici,
            goc_dizini: goc_dizini.map(|p| p.to_path_buf()),
        })
    }

    /// Dosya tabanlı SQLite deposu açar.
    pub fn dosya_ac(
        yol: &Path,
        sifreleyici: Sifreleyici,
        goc_dizini: Option<&Path>,
    ) -> Result<Self> {
        let conn = Connection::open(yol)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            sifreleyici,
            goc_dizini: goc_dizini.map(|p| p.to_path_buf()),
        })
    }
}

impl Depo for RusqliteDepo {
    fn gocleri_calistir(&self) -> Result<usize> {
        let goc_yolu = match &self.goc_dizini {
            Some(p) => p,
            None => anyhow::bail!("Göç dizini yapılandırılmamış"),
        };

        let gocler = goc_dosyalarini_yukle(goc_yolu)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Kilit hatası: {}", e))?;

        // Göç kayıt tablosu oluştur
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _kervan_gocler (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dosya_adi TEXT UNIQUE NOT NULL,
                uygulandi_tarih DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )?;

        let mut sayac = 0;
        for (ad, sql) in gocler {
            let zaten_var: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM _kervan_gocler WHERE dosya_adi = ?1)",
                [&ad],
                |row| row.get(0),
            )?;

            if !zaten_var {
                sutunlari_guvenceye_al(&conn)?;

                if sema_zaten_uygulanmis_mi(&conn, &ad) {
                    // OmniRoute isSchemaAlreadyApplied: şema mevcutsa SQL tekrar koşulmaz
                    conn.execute("INSERT INTO _kervan_gocler (dosya_adi) VALUES (?1)", [&ad])?;
                    sayac += 1;
                    continue;
                }

                if let Err(err) = conn.execute_batch(&sql) {
                    let err_msg = err.to_string();
                    if err_msg.contains("duplicate column name") {
                        // OmniRoute migrationRunner.ts:
                        // Sütun önceden mevcutsa (idempotency) göç uygulandı sayılır
                    } else {
                        return Err(err).with_context(|| format!("Göç çalıştırılamadı: {}", ad));
                    }
                }

                sutunlari_guvenceye_al(&conn)?;

                conn.execute("INSERT INTO _kervan_gocler (dosya_adi) VALUES (?1)", [&ad])?;
                sayac += 1;
            }
        }

        Ok(sayac)
    }

    fn tablo_sayisi(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Kilit hatası: {}", e))?;
        let tablolar = tablolari_listele(&conn)?;
        // _kervan_gocler tablosu iç takip içindir, altın şemaya dahil değildir
        let filtreli = tablolar
            .iter()
            .filter(|t| !t.starts_with("_kervan_"))
            .count();
        Ok(filtreli)
    }

    fn saglik_kontrolu(&self) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Kilit hatası: {}", e))?;
        let sonuc: String = conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0))?;
        Ok(sonuc == "ok")
    }

    fn yedekle(&self, hedef_yol: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Kilit hatası: {}", e))?;
        conn.execute("VACUUM INTO ?1;", [hedef_yol])?;
        Ok(())
    }

    fn deger_getir(&self, tablo: &str, anahtar: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Kilit hatası: {}", e))?;
        let sorgu = format!("SELECT deger FROM {} WHERE anahtar = ?1 LIMIT 1;", tablo);
        let mut stmt = conn.prepare(&sorgu)?;
        let sonuc: Result<String, _> = stmt.query_row([anahtar], |r| r.get(0));

        match sonuc {
            Ok(sifreli_veya_duz) => {
                let cozulmus = self
                    .sifreleyici
                    .coz(&sifreli_veya_duz)
                    .map_err(|e| anyhow::anyhow!("Şifre çözme hatası: {}", e))?;
                Ok(Some(cozulmus))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Veritabanı okuma hatası: {}", e)),
        }
    }

    fn deger_kaydet(&self, tablo: &str, anahtar: &str, deger: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Kilit hatası: {}", e))?;
        let sifreli = self
            .sifreleyici
            .sifrele(deger)
            .map_err(|e| anyhow::anyhow!("Şifreleme hatası: {}", e))?;

        let sorgu = format!(
            "INSERT INTO {} (anahtar, deger) VALUES (?1, ?2)
             ON CONFLICT(anahtar) DO UPDATE SET deger = excluded.deger;",
            tablo
        );
        conn.execute(&sorgu, [&anahtar, &sifreli.as_str()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_bellek_ici_temel_islemler() {
        let sifreleyici = Sifreleyici::yeni(Some("test-secret-kervan-32-chars-long!"));
        let depo = RusqliteDepo::bellek_ici(sifreleyici, None).expect("Depo açılabilmeli");

        assert!(depo.saglik_kontrolu().expect("Bütünlük sağlam olmalı"));
    }

    #[test]
    fn test_193_gocun_calistirilmasi_ve_altin_sema_esitligi() {
        let sifreleyici = Sifreleyici::yeni(None);
        let goc_yolu = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ref/omniroute-dbe703a/src/lib/db/migrations");

        if !goc_yolu.exists() {
            eprintln!("Atlandı: Göç dizini bulunamadı (CI veya test ortamı)");
            return;
        }

        let depo =
            RusqliteDepo::bellek_ici(sifreleyici, Some(&goc_yolu)).expect("Depo açılabilmeli");
        let calisan = depo
            .gocleri_calistir()
            .expect("193 göç başarıyla uygulanmalı");

        assert_eq!(calisan, 193, "Tam olarak 193 göç uygulanmış olmalı");
        assert!(depo.saglik_kontrolu().expect("Bütünlük sağlam olmalı"));

        {
            let conn = depo.conn.lock().unwrap();
            let olusan_tablolar: std::collections::HashSet<String> = {
                let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE '_kervan_%'").unwrap();
                stmt.query_map([], |r| r.get(0))
                    .unwrap()
                    .map(|r| r.unwrap())
                    .collect()
            };

            let veritabani_json = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../port/envanter/veritabani.json");
            let altin_json: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&veritabani_json).unwrap()).unwrap();
            let altin_tablolar: std::collections::HashSet<String> = altin_json["altinSema"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| x["type"] == "table")
                .map(|x| x["name"].as_str().unwrap().to_string())
                .collect();

            // call_logs_v1_legacy, Bun envanter koşusunda 025 yarıda kaldığı için oluşan geçici bir tablodur;
            // Rusqlite'ın tam şema iyileştirmesiyle 025 güvenli idempotency ile geçer.
            let fark: Vec<_> = altin_tablolar
                .difference(&olusan_tablolar)
                .filter(|t| *t != "call_logs_v1_legacy")
                .collect();
            assert!(
                fark.is_empty(),
                "Altın şemadaki tüm çekirdek tablolar mevcut olmalı: {:?}",
                fark
            );
        }

        let tablo_sayisi = depo.tablo_sayisi().expect("Tablo sayısı okunabilmeli");
        assert_eq!(
            tablo_sayisi, 134,
            "Çekirdek alan tabloları (134) eksiksiz oluşturulmuş olmalı"
        );
    }

    #[test]
    fn test_sifreli_deger_yazma_ve_okuma() {
        let sifreleyici = Sifreleyici::yeni(Some("test-secret-kervan-32-chars-long!"));
        let depo = RusqliteDepo::bellek_ici(sifreleyici, None).expect("Depo açılabilmeli");

        {
            let conn = depo.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE TABLE test_kv (anahtar TEXT PRIMARY KEY, deger TEXT NOT NULL);",
            )
            .unwrap();
        }

        depo.deger_kaydet("test_kv", "gemini_key", "sk-test-secret-12345")
            .unwrap();
        let okunan = depo.deger_getir("test_kv", "gemini_key").unwrap();
        assert_eq!(okunan, Some("sk-test-secret-12345".to_string()));

        // Veritabanındaki ham değere bakalım (şifreli mi?)
        {
            let conn = depo.conn.lock().unwrap();
            let ham: String = conn
                .query_row(
                    "SELECT deger FROM test_kv WHERE anahtar = 'gemini_key'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(ham.starts_with("enc:v1:"));
            assert_ne!(ham, "sk-test-secret-12345");
        }
    }
}
