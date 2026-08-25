//! Surec agaci: Kervan olunce tarayicilar da olsun.
//!
//! SORUN: `Session::drop` yalnizca duzgun kapanista calisir. Kervan zorla
//! oldurulunce (TerminateProcess — Ctrl+C degil, gorev yoneticisi ya da
//! `Stop-Process -Force`) geckodriver ve onun actigi tarayici arkada kalir.
//! Bunlar profili KILITLI tutar: sonraki calistirmada Zen "zaten calisiyor"
//! penceresi acar, o pencere modal oldugu icin surec orada asili kalir.
//! Olculdu — hic Kervan calismazken iki artik Zen sureci bir profili tutuyordu.
//!
//! COZUM: Windows'un kendi mekanizmasi. Surec bir "job object"e alinir,
//! KILL_ON_JOB_CLOSE isaretlenir; cocuklar uyeligi miras alir. Kervan nasil
//! olurse olsun (zorla dahil) isletim sistemi tutamaci kapatir ve isteki tum
//! surecleri oldurur. Kullanici alanindan yapilabilecek daha guvenilir bir sey yok:
//! zorla oldurulen bir surec hicbir temizlik kodu calistiramaz.

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
