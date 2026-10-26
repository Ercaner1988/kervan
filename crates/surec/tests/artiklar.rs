//! Gercek surec sinamalari: supurme ve kilit temizligi.
//!
//! Ayri dosyada cunku `CARGO_BIN_EXE_*` yalnizca butunlesme sinamalarinda tanimli;
//! kutuphane icindeki birim sinamasi yardimci ikiliyi bulamaz.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kervan_surec::{Artik, artiklari_supur, kilitleri_temizle, supur_ile, tara_ile};

/// Kendi actigimiz bir surec, komut satirinda bizim profil kokumuz gecerek baslar;
/// supurme onu bulmali ve oldurmelidir.
///
/// `uyuyan` yardimcisi kullaniliyor cunku `-profile` tasiyan baska bir masum surec
/// yok, sahte komut satiri kurmanin kullanici alaninda yolu da yok.
#[cfg(windows)]
#[test]
fn sahipsiz_surec_bulunur_ve_oldurulur() {
    let kok = std::env::temp_dir().join("kervan-surec-sinama");
    let profil = kok.join("sinama@ornek.com");
    std::fs::create_dir_all(&profil).expect("profil dizini");

    let mut cocuk = Command::new(env!("CARGO_BIN_EXE_uyuyan"))
        .arg("-profile")
        .arg(&profil)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("uyuyan baslatilamadi");
    let pid = cocuk.id();

    // Tarama: bizim cocugumuz, atalarinda baska bir kervan yok -> sahipsiz.
    let adlar = ["uyuyan.exe"];
    let tarama = tara_ile(&kok, &adlar);
    assert!(
        tarama.sahipsiz.iter().any(|a| a.pid == pid),
        "sahipsiz surec bulunamadi: {:?}",
        tarama.sahipsiz
    );
    assert!(
        tarama.baskasinin.is_empty(),
        "kendi cocugumuz baskasinin sayilmamali: {:?}",
        tarama.baskasinin
    );

    let rapor = supur_ile(&kok, &adlar);
    assert!(rapor.kapatilan >= 1, "kapatilan={}", rapor.kapatilan);
    assert!(rapor.kapanmayan.is_empty(), "{:?}", rapor.kapanmayan);
    let oldu = cocuk.try_wait().map(|s| s.is_some()).unwrap_or(true);
    assert!(oldu, "surec hala yasiyor");

    let _ = cocuk.kill();
    let _ = std::fs::remove_dir_all(&kok);
}

/// Kilit yalnizca sahipsiz profilden silinir; baska bir Kervan'in yasayan
/// tarayicisinin kilidine dokunulmaz.
#[cfg(windows)]
#[test]
fn kilit_yalnizca_sahipsiz_profilden_silinir() {
    let kok = std::env::temp_dir().join("kervan-kilit-sinama");
    let sahipsiz = kok.join("sahipsiz@ornek.com");
    let yasayan = kok.join("yasayan@ornek.com");
    for d in [&sahipsiz, &yasayan] {
        std::fs::create_dir_all(d).expect("profil dizini");
        std::fs::write(d.join("parent.lock"), b"x").expect("kilit");
    }
    // Yasayan tarayici taklidi: komutu o profili gosteriyor.
    let taklit = Artik {
        pid: 1,
        komut: format!(r#"zen.exe -profile "{}""#, yasayan.display()),
    };
    let silinen = kilitleri_temizle(&kok, std::slice::from_ref(&taklit));
    assert_eq!(silinen, 1, "yalnizca sahipsiz profilin kilidi silinmeli");
    assert!(!sahipsiz.join("parent.lock").exists());
    assert!(yasayan.join("parent.lock").exists());
    let _ = std::fs::remove_dir_all(&kok);
}

/// Kok yoksa (ilk kurulum) supurme paniklememeli ve is yapmamis olmali.
#[test]
fn olmayan_kok_sessiz_gecer() {
    let yok: PathBuf = std::env::temp_dir().join("kervan-olmayan-kok-12345");
    let rapor = artiklari_supur(&yok);
    assert_eq!(rapor.kapatilan, 0);
    assert_eq!(rapor.sahipsiz, 0);
    assert!(rapor.ozet().is_none());
    assert_eq!(kilitleri_temizle(Path::new(&yok), &[]), 0);
}
