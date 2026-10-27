//! Surec katmani: Kervan'in actigi tarayicilar onunla birlikte olmeli.
//!
//! IKI KATMAN var, cunku tek katman yetmedi:
//!
//! 1. **Job object** (`agaci_bagla`): Kervan bir is nesnesine girer,
//!    `KILL_ON_JOB_CLOSE` isaretlenir; cocuklar uyeligi miras alir. Kervan nasil
//!    olurse olsun (zorla dahil) isletim sistemi tutamaci kapatir ve isteki tum
//!    surecleri oldurur. Kullanici alanindan yapilabilecek daha guvenilir bir sey
//!    yok: zorla oldurulen bir surec hicbir temizlik kodu calistiramaz.
//!    Olculdu (2026-10-26): sahibi `Stop-Process -Force` ile oldurulunce 12 Zen
//!    sureci de oldu.
//!
//! 2. **Supurme** (`artiklari_supur`): is nesnesi yalnizca BUGUNDEN sonrasini
//!    kurtarir. Is nesnesi kurulamazsa (izin, eski surum, `AssignProcessToJobObject`
//!    hatasi) ya da tarayici iste uyeligi birakip yeniden dogarsa artik yine kalir.
//!    Artik, profili KILITLI tutar: sonraki kosu "zaten calisiyor" modal penceresinde
//!    asili kalir. Acilista komut satirinda kendi profil kokumuz gecen tarayicilari
//!    bulup kapatiyoruz; ardindan kimsenin tutmadigi kilit dosyalarini siliyoruz.
//!
//! BASKASININ TARAYICISINA DOKUNMAYIZ: bir Zen'in atalarinda CANLI bir
//! `kervan.exe` varsa (bizden baska), o tarayici bizim artik degil o sunucunun
//! malidir; atlanir ve profili kilitli birakilir. Kullanicinin kendi Zen'i zaten
//! `-profile <bizim kok>` tasimadigi icin eslesmez.
//!
//! KOMUT SATIRI NASIL OKUNUYOR: `GetProcessInformation(ProcessCommandLineInformation)`
//! hata 87 (ERROR_INVALID_PARAMETER) veriyor — o sinif `PROCESS_INFORMATION_CLASS`
//! icinde degil, `PROCESSINFOCLASS` icinde 60 numaradir. Dogrusu
//! `NtQueryInformationProcess(60)`: olculdu, 8952'nin satirini dondurdu. Bu yuzden
//! `Wdk_System_Threading` feature'i gerekir.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 1. Job object
// ---------------------------------------------------------------------------

/// Bu sureci ve tum cocuklarini tek bir olum kaderine baglar.
///
/// Basarisiz olursa program DURMAZ: bu bir emniyet agi, isin kendisi degil.
/// Ag kurulamadiysa soyleyip devam etmek, ag yok diye calismamaktan iyidir.
#[cfg(windows)]
pub fn agaci_bagla() {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let is = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if is.is_null() {
            eprintln!("uyari: job object olusturulamadi; zorla oldurulurse tarayici sizabilir");
            return;
        }
        let mut bilgi: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        bilgi.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            is,
            JobObjectExtendedLimitInformation,
            &bilgi as *const _ as *const std::ffi::c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            eprintln!("uyari: job object ayarlanamadi; zorla oldurulurse tarayici sizabilir");
            return;
        }
        if AssignProcessToJobObject(is, GetCurrentProcess()) == 0 {
            eprintln!("uyari: surec job object'e alinamadi; zorla oldurulurse tarayici sizabilir");
            return;
        }
        // Tutamak BILEREK kapatilmiyor: `is` ham bir HANDLE, kendi kendine kapanmaz.
        // Kapatsaydik is nesnesi hemen yok olur ve koruma o anda biterdi. Surec
        // olunce isletim sistemi tutamagi kapatir, iste o an tum agac oldurulur.
        let _ = is;
    }
}

#[cfg(not(windows))]
pub fn agaci_bagla() {
    // Windows disinda Kervan denenmedi; process group / prctl karsiligi gerekirse
    // buraya gelir.
}

// ---------------------------------------------------------------------------
// 2. Komut satiri: saf, sinanabilir ayristirma
// ---------------------------------------------------------------------------

/// Komut satirini belirteclere ayirir; `"..."` icindeki bosluklar bolunmez.
///
/// NEDEN KENDI AYRISTIRICI: Windows yolu bosluk tasiyabilir
/// (`C:\Users\Ali Veli\...`) ve tarayici yolu tirnaklanir. `split_whitespace`
/// bu yolu ikiye boler, eslesme sessizce kacar. Kacmanin bedeli: artik surec
/// yasamaya devam eder, kilit silinmez ve sonraki kosu modal pencerede asili kalir.
fn belirtecler(satir: &str) -> Vec<String> {
    let mut sonuc = Vec::new();
    let mut parca = String::new();
    let mut tirnak_icinde = false;
    for c in satir.chars() {
        match c {
            '"' => tirnak_icinde = !tirnak_icinde,
            c if c.is_whitespace() && !tirnak_icinde => {
                if !parca.is_empty() {
                    sonuc.push(std::mem::take(&mut parca));
                }
            }
            c => parca.push(c),
        }
    }
    if !parca.is_empty() {
        sonuc.push(parca);
    }
    sonuc
}

/// Komut satirindaki `-profile <dizin>` degerini verir. Yoksa `None`.
///
/// Iki yazim da kabul: `-profile <yol>` (Kervan'in kullandigi) ve
/// `-profile=<yol>` (Firefox belgelerinde gecen).
pub fn profil_dizini(komut_satiri: &str) -> Option<PathBuf> {
    let b = belirtecler(komut_satiri);
    for (i, t) in b.iter().enumerate() {
        if t.eq_ignore_ascii_case("-profile") {
            return b.get(i + 1).map(PathBuf::from);
        }
        // `t.get(..9)` kullaniliyor: `&t[..9]` cok baytli karakterin ortasini
        // kesebilir ve panikler. Yol Turkce karakterle baslayabilir.
        if t.get(..9)
            .is_some_and(|p| p.eq_ignore_ascii_case("-profile="))
        {
            return t.get(9..).filter(|p| !p.is_empty()).map(PathBuf::from);
        }
    }
    None
}

/// Windows yol karsilastirmasi icin duz bicim: ASCII kucuk harf, `/` -> `\`,
/// sondaki ayirac atilir.
///
/// NEDEN: Windows yollari harf buyuklugune duyarsiz ve ayni dizin `C:\a\b` ile
/// `C:/a/b` olarak iki turlu yazilabilir. Duz bicim olmadan eslesme kacar.
///
/// NEDEN `to_ascii_lowercase`, `katla` DEGIL: NTFS harf eslestirmesini kendi
/// upcase tablosuyla yapar ve bu tablo `ü`/`Ü` gibi harfleri eslestirmez, ama
/// `katla` `ü`'yu `u`'ya indirir — o zaman `C:\profiles\masaustu` ile
/// `C:\profiles\Masaüstü` ayni sayilir ve YANLIS dizin eslesir. Yol eslestirmesi
/// Turkce metin aramasi degil, bayt kimligi isidir.
fn duz(yol: &Path) -> String {
    let s = yol.to_string_lossy().replace('/', "\\");
    s.trim_end_matches('\\').to_ascii_lowercase()
}

/// Komut satiri Kervan'in profil kokune ait bir tarayici mi?
///
/// Kokun KENDISI ya da kok disi bir dizin eslesmez; yalnizca kokun altindaki bir
/// profil dizini eslesir. Ayirac kontrolu (`\`) bu yuzden var: `...\profiles-yedek`
/// `...\profiles` ile baslar ama altinda degildir.
pub fn profil_kokune_ait_mi(komut_satiri: &str, kok: &Path) -> bool {
    let Some(dizin) = profil_dizini(komut_satiri) else {
        return false;
    };
    let d = duz(&dizin);
    let k = duz(kok);
    d.len() > k.len() && d.starts_with(&k) && d.as_bytes()[k.len()] == b'\\'
}

// ---------------------------------------------------------------------------
// 3. Surec listesi ve komut satiri (Windows)
// ---------------------------------------------------------------------------

/// Bulunan artik bir tarayici sureci.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artik {
    pub pid: u32,
    pub komut: String,
}

/// Tarama sonucu.
#[derive(Debug, Default)]
pub struct Tarama {
    /// Sahibi olu; kapatilabilir.
    pub sahipsiz: Vec<Artik>,
    /// Bizim profilimizi kullaniyor ama atalarinda CANLI bir Kervan var; dokunma.
    pub baskasinin: Vec<Artik>,
}

/// Supurme sonucu.
#[derive(Debug, Default)]
pub struct Rapor {
    pub sahipsiz: usize,
    pub kapatilan: usize,
    pub atlanan: usize,
    /// Kapatilamayanlar: (pid, neden).
    pub kapanmayan: Vec<(u32, String)>,
    pub silinen_kilit: usize,
}

impl Rapor {
    /// Tek satirlik ozet; hicbir sey bulunmadiysa `None` (acilista sessiz kal).
    pub fn ozet(&self) -> Option<String> {
        if self.sahipsiz == 0 && self.silinen_kilit == 0 {
            return None;
        }
        let mut s = format!(
            "artik tarayici: {} sahipsiz, {} kapatildi, {} atlandi (baska Kervan), {} kilit silindi",
            self.sahipsiz, self.kapatilan, self.atlanan, self.silinen_kilit
        );
        for (pid, neden) in &self.kapanmayan {
            s.push_str(&format!("\n  kapatilamadi: pid {pid} — {neden}"));
        }
        Some(s)
    }
}

#[cfg(windows)]
mod win {
    use super::{Artik, Tarama, duz, profil_kokune_ait_mi};
    use std::path::Path;

    use windows_sys::Wdk::System::Threading::{
        NtQueryInformationProcess, ProcessCommandLineInformation,
    };
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, INVALID_HANDLE_VALUE, UNICODE_STRING,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    /// (pid, ust pid, exe adi).
    pub(super) type Satir = (u32, u32, String);

    pub(super) fn surecler() -> Vec<Satir> {
        let mut sonuc = Vec::new();
        unsafe {
            let anlik = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if anlik == INVALID_HANDLE_VALUE {
                return sonuc;
            }
            let mut girdi: PROCESSENTRY32W = std::mem::zeroed();
            girdi.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut devam = Process32FirstW(anlik, &mut girdi) != 0;
            while devam {
                let ad = String::from_utf16_lossy(
                    &girdi.szExeFile[..girdi
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(girdi.szExeFile.len())],
                );
                sonuc.push((girdi.th32ProcessID, girdi.th32ParentProcessID, ad));
                devam = Process32NextW(anlik, &mut girdi) != 0;
            }
            CloseHandle(anlik);
        }
        sonuc
    }

    /// Bir surecin komut satiri; okunamazsa `None`.
    pub fn komut_satiri(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            // 32 KiB metin: Firefox `-contentproc` satirlari uzun ama bu sinirin
            // altinda. Yetmezse API hata verir, sessizce kirpilmis metin vermez.
            let metin_kapasite = 32 * 1024usize;
            let boy = std::mem::size_of::<UNICODE_STRING>() + metin_kapasite * 2;
            let mut tampon = vec![0u8; boy];
            let us = tampon.as_mut_ptr() as *mut UNICODE_STRING;
            (*us).Length = 0;
            (*us).MaximumLength = (metin_kapasite * 2) as u16;
            (*us).Buffer = tampon
                .as_mut_ptr()
                .add(std::mem::size_of::<UNICODE_STRING>()) as *mut u16;
            let mut donen: u32 = 0;
            let durum = NtQueryInformationProcess(
                h,
                ProcessCommandLineInformation,
                tampon.as_mut_ptr() as *mut _,
                tampon.len() as u32,
                &mut donen,
            );
            CloseHandle(h);
            if durum < 0 {
                return None;
            }
            let us = tampon.as_ptr() as *const UNICODE_STRING;
            let uzunluk = (*us).Length as usize / 2;
            if (*us).Buffer.is_null() || uzunluk == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                (*us).Buffer,
                uzunluk,
            )))
        }
    }

    fn tarayici_mi(exe: &str, adlar: &[&str]) -> bool {
        adlar.iter().any(|a| exe.eq_ignore_ascii_case(a))
    }

    /// Atalar zincirinde CANLI bir kervan var mi? Varsa bu tarayici bizim artik
    /// degil, o Kervan'in malidir.
    ///
    /// Zinciri pid -> ust pid ile yururuz; kendi pid'imizi "baskasi" saymayiz.
    fn sahibi_yasiyor(liste: &[Satir], ust: u32) -> bool {
        let ben = std::process::id();
        let mut simdiki = ust;
        for _ in 0..64 {
            if simdiki == 0 || simdiki == ben {
                return false;
            }
            let Some((_, ustu, exe)) = liste.iter().find(|(p, _, _)| *p == simdiki) else {
                return false;
            };
            if exe.eq_ignore_ascii_case("kervan.exe") {
                return true;
            }
            // Zincir kendine dondu: bozuk tablo, dur.
            if *ustu == simdiki {
                return false;
            }
            simdiki = *ustu;
        }
        false
    }

    pub fn tara(kok: &Path, adlar: &[&str]) -> Tarama {
        let liste = surecler();
        let mut sonuc = Tarama::default();
        for (pid, ust, exe) in &liste {
            if *pid == std::process::id() || !tarayici_mi(exe, adlar) {
                continue;
            }
            let Some(komut) = komut_satiri(*pid) else {
                continue;
            };
            if !profil_kokune_ait_mi(&komut, kok) {
                continue;
            }
            let kayit = Artik { pid: *pid, komut };
            if sahibi_yasiyor(&liste, *ust) {
                sonuc.baskasinin.push(kayit);
            } else {
                sonuc.sahipsiz.push(kayit);
            }
        }
        sonuc
    }

    /// Sureci oldurur ve olumunu bekler. `Err` icinde neden var.
    ///
    /// DUZGUN KAPATMA DENENMIYOR (WM_CLOSE): sahibi zaten olu, pencereyi kapatacak
    /// kimse yok; tarayici "oturumu kaydet?" gibi modal bir soru acarsa surec asili
    /// kalir ve supurme ise yaramaz. Job object da ayni sertlikte oldurur. Bu
    /// profiller otomasyon profilidir, icinde insanin kaydedecek isi yoktur.
    pub fn oldur(pid: u32) -> Result<(), String> {
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::Threading::{
            PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
        };
        unsafe {
            let h = OpenProcess(
                PROCESS_TERMINATE | PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            );
            if h.is_null() {
                return Err(format!("OpenProcess basarisiz (hata {})", GetLastError()));
            }
            let ok = TerminateProcess(h, 1);
            if ok == 0 {
                let hata = GetLastError();
                CloseHandle(h);
                return Err(format!("TerminateProcess basarisiz (hata {hata})"));
            }
            let bekle = WaitForSingleObject(h, 3000);
            CloseHandle(h);
            if bekle == WAIT_OBJECT_0 {
                Ok(())
            } else {
                // Bazi antivirus/askida kalan surecler 3 sn icinde olmez; yalan
                // soylememek icin bunu basarisiz sayiyoruz.
                Err("3 sn icinde olmedi".to_string())
            }
        }
    }

    /// Kimsenin tutmadigi profil kilitlerini siler; silinen dosya sayisini verir.
    ///
    /// Yalnizca `yasayan` listesinde gecmeyen profillere dokunur: baska bir
    /// Kervan'in acik tarayicisinin kilidi silinirse o Kervan'in profili iki kez
    /// acilabilir hale gelir — yani duzeltmek istedigimiz hatanin tersi.
    pub fn kilitleri_temizle(kok: &Path, yasayan: &[Artik]) -> usize {
        let Ok(girdiler) = std::fs::read_dir(kok) else {
            return 0;
        };
        let yasayan_duz: Vec<String> = yasayan.iter().map(|a| duz(Path::new(&a.komut))).collect();
        let mut sayi = 0;
        for girdi in girdiler.flatten() {
            let dir = girdi.path();
            if !dir.is_dir() {
                continue;
            }
            let d = duz(&dir);
            if yasayan_duz.iter().any(|k| k.contains(&d)) {
                continue;
            }
            for ad in ["parent.lock", ".parentlock", "lock"] {
                let dosya = dir.join(ad);
                // `lock` bir sembolik bag olabilir; `remove_file` bagin kendisini siler.
                if dosya.symlink_metadata().is_ok() && std::fs::remove_file(&dosya).is_ok() {
                    sayi += 1;
                }
            }
        }
        sayi
    }
}

/// Supurmenin hedefledigi surec adlari. Kervan'in actigi tarayicilar bunlar.
///
/// NEDEN AD SUZGECI VAR: komut satiri eslesmesi tek basina yetmez — `-profile`
/// tasiyan baska araclar da olabilir. Adi da sinamak, yanlis sureci oldurme riskini
/// ortadan kaldirir. Sinamalar kendi yardimci ikililerini bu listeyle verir.
pub const TARAYICI_ADLARI: &[&str] = &["zen.exe", "firefox.exe"];

#[cfg(windows)]
pub use win::{kilitleri_temizle, komut_satiri};

/// Artik tarayicilari bulur, kapatir ve sahipsiz kilitleri siler.
///
/// Idempotent: artik yoksa hicbir sey yapmaz ve sessiz doner.
pub fn artiklari_supur(kok: &Path) -> Rapor {
    supur_ile(kok, TARAYICI_ADLARI)
}

/// `artiklari_supur`'un suzgec alan bicimi: sinamalar yardimci ikiliyi
/// `TARAYICI_ADLARI` listesine eklemek zorunda kalmasin diye ayri.
pub fn supur_ile(kok: &Path, adlar: &[&str]) -> Rapor {
    #[cfg(not(windows))]
    {
        let _ = (kok, adlar);
        return Rapor::default();
    }
    #[cfg(windows)]
    {
        let tarama = win::tara(kok, adlar);
        let mut rapor = Rapor {
            sahipsiz: tarama.sahipsiz.len(),
            atlanan: tarama.baskasinin.len(),
            ..Rapor::default()
        };
        for a in &tarama.sahipsiz {
            match win::oldur(a.pid) {
                Ok(()) => rapor.kapatilan += 1,
                Err(neden) => rapor.kapanmayan.push((a.pid, neden)),
            }
        }
        // Kilit temizligi EN SON: once surecler olmeli, yoksa kilit hala tutuluyor
        // olabilir ve silmek yanlis olur. Kapatilamayanlar da "yasayan" sayilir.
        let mut yasayan = tarama.baskasinin;
        for (pid, _) in &rapor.kapanmayan {
            if let Some(a) = tarama.sahipsiz.iter().find(|a| a.pid == *pid) {
                yasayan.push(a.clone());
            }
        }
        rapor.silinen_kilit = win::kilitleri_temizle(kok, &yasayan);
        rapor
    }
}

/// Yalnizca tarama; hicbir seyi kapatmaz (teshis ve sinama icin).
pub fn tara_ile(kok: &Path, adlar: &[&str]) -> Tarama {
    #[cfg(not(windows))]
    {
        let _ = (kok, adlar);
        Tarama::default()
    }
    #[cfg(windows)]
    {
        win::tara(kok, adlar)
    }
}

#[cfg(test)]
mod sinama {
    use super::*;

    fn kok() -> PathBuf {
        PathBuf::from(r"C:\Users\ercan\AppData\Local\kervan\profiles")
    }

    #[test]
    fn otomasyon_satiri_eslesir() {
        let komut = r#""C:\Program Files\Zen Browser\zen.exe" -no-remote -profile C:\Users\ercan\AppData\Local\kervan\profiles\ercan@gmail.com"#;
        assert!(profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn tirnakli_ve_bosluklu_yol_eslesir() {
        let komut = r#""C:\Program Files\Zen Browser\zen.exe" -profile "C:\Users\Ali Veli\kervan\profiles\a@b.com""#;
        assert!(profil_kokune_ait_mi(
            komut,
            Path::new(r"C:\Users\Ali Veli\kervan\profiles")
        ));
    }

    #[test]
    fn egittir_isareti_yazimi_eslesir() {
        let komut = r#"zen.exe -profile=C:/Users/ercan/AppData/Local/kervan/profiles/a@b.com"#;
        assert!(profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn kullanicinin_kendi_zen_satiri_eslesmez() {
        // Olculdu: kullanicinin Zen'i tam bu satirla calisiyor (2026-10-26).
        let komut = r#""C:\Program Files\Zen Browser\zen.exe" https://x.com/home"#;
        assert!(!profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn icerik_sureci_satiri_eslesmez() {
        // `-contentproc` cocuklari `-profile` tasimaz; kok surec oldurulunce
        // onlar da gider. Onlari ayrica hedeflemek gerekmez.
        let komut = r#""C:\Program Files\Zen Browser\zen.exe" -contentproc -parentPid 8952 -appDir "C:\Program Files\Zen Browser\browser" - 1 gpu"#;
        assert!(!profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn benzer_adli_komsu_dizin_eslesmez() {
        // `profiles-yedek` ile `profiles` ayni onekle baslar; ayirac kontrolu
        // olmasa yanlis tarayici oldurulurdu.
        let komut =
            r#"zen.exe -profile C:\Users\ercan\AppData\Local\kervan\profiles-yedek\a@b.com"#;
        assert!(!profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn kokun_kendisi_eslesmez() {
        let komut = r#"zen.exe -profile C:\Users\ercan\AppData\Local\kervan\profiles"#;
        assert!(!profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn buyuk_kucuk_harf_ve_egik_cizgi_ayrimi_yok() {
        let komut = r#"zen.exe -profile c:/users/ERCAN/appdata/local/KERVAN/profiles/a@b.com"#;
        assert!(profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn varsayilan_profil_yolu_eslesmez() {
        let komut = r#"firefox.exe -profile C:\Users\ercan\AppData\Roaming\Mozilla\Firefox\Profiles\abcd.default"#;
        assert!(!profil_kokune_ait_mi(komut, &kok()));
    }

    #[test]
    fn profil_belirteci_yoksa_none() {
        assert!(profil_dizini(r#"zen.exe https://x.com/home"#).is_none());
        assert!(profil_dizini(r#"zen.exe -profile"#).is_none());
        assert_eq!(
            profil_dizini(r#"zen.exe -profile C:\a\b"#),
            Some(PathBuf::from(r"C:\a\b"))
        );
    }
}
