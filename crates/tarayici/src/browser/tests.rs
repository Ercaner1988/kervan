use super::*;

/// Buyuk/kucuk harf ve bosluk farkini yutan icerme sinamasi.
///
/// Menudeki metin "3.1 PRO", " Yüksek ", "Gemini\nOtomatik" gibi gelebiliyor;
/// ham `contains` bunlarin cogunu kacirir ve var olan secenek "menude yok"
/// sayilirdi.
#[test]
fn model_metni_normalize_edilerek_karsilastirilir() {
    assert!(metin_iceriyor_mu("3.1 PRO", "3.1 Pro"));
    assert!(metin_iceriyor_mu("Gemini\nOtomatik", "otomatik"));
    assert!(metin_iceriyor_mu("  Yüksek  ", "yüksek"));
    assert!(!metin_iceriyor_mu("Düşük", "yüksek"));
    // Ortak `katla` sandigi aksanlari da atar: "yuksek" = "Yüksek", "isik" = "IŞIK".
    assert!(metin_iceriyor_mu("Yüksek", "yuksek"));
    assert!(metin_iceriyor_mu("IŞIK", "isik"));
}

/// Profil dizini hesap adina gore ayrilmali — yalitimin tamami buna dayaniyor.
/// Bozulursa hesaplar birbirinin cerezini gorur, ki OmniRoute'un paylasilan
/// tarayici baglaminda yasadigi sorun tam olarak budur.
#[test]
fn profil_dizinleri_hesaba_gore_ayrilir() {
    let root = Path::new("C:\\kervan\\profiles");
    assert_ne!(root.join("hesap-a"), root.join("hesap-b"));
    // ayni ad ayni dizini vermeli, yoksa oturum kalici olmaz
    assert_eq!(root.join("hesap-a"), root.join("hesap-a"));
}

/// Gmail takma adlarindaki `+` kodlanmazsa sunucuda BOSLUGA donusur ve yanlis
/// hesabin giris ekrani acilir.
#[test]
fn arti_isareti_kodlanir() {
    let p = Provider {
        ad: "deneme".into(),
        url: "x".into(),
        selectors: Selectors {
            prompt_input: "a".into(),
            login_hint: "b".into(),
            response: "c".into(),
        },
        giris_kalibi: "https://o/?Email={eposta}".into(),
        cloudflare: false,
        model: None,
    };
    assert_eq!(
        p.giris_url("ad+etiket@gmail.com"),
        "https://o/?Email=ad%2Betiket@gmail.com"
    );
}

/// ASIL HATA BUYDU: perplexity'de istemin yalnizca ilk karakteri kutuya girdi,
/// Enter'a basilinca site "S" harfini aratti ve ansiklopedik bir cevap dondu.
/// Yani eksik istem, YANLIS SORUYA DOGRU GORUNEN cevap uretiyor. Kisalmis metin
/// asla "yazildi" sayilmamali.
#[test]
fn eksik_yazilan_istem_kabul_edilmez() {
    let istem = "Sadece 'elma' yaz, baska hicbir sey yazma.";
    assert!(!metin_esit_mi("S", istem), "tek harf istem sayilmamali");
    assert!(!metin_esit_mi("", istem), "bos kutu istem sayilmamali");
    assert!(
        !metin_esit_mi("Sadece 'elma' yaz", istem),
        "kirpilmis metin kabul edilmemeli"
    );
    assert!(metin_esit_mi(istem, istem));
}

/// Teshis cumlesi, sonraki adimi belirledigi icin ayrimin kendisi sinaniyor:
/// KIRPMIS -> tavani dusur, ESIT -> sadelestirmeyi genislet.
/// OLCULEN iki ariza da burada sinaniyor; ikisi de z.ai'dan geldi.
#[test]
fn dusunme_gurultusu_ayiklanir() {
    // (a) cevabin basindaki gurultu
    assert_eq!(cevabi_ayikla("Thought Process\nAnkara"), "Ankara");
    // (b) henuz cevap yok: ayiklaninca geriye bir sey kalmamali
    assert_eq!(cevabi_ayikla("Thinking... Skip").trim(), "");
    assert_eq!(cevabi_ayikla("Thinking...\nSkip").trim(), "");
    // Gercek icerik tasiyan satira DOKUNULMAMALI
    assert_eq!(
        cevabi_ayikla("Thinking about it: Ankara"),
        "Thinking about it: Ankara"
    );
    assert_eq!(cevabi_ayikla("Ankara"), "Ankara");
}

/// "Thinking... Skip" ayiklanmadan once KARARLI gorunuyordu — yani cevap
/// beklemek yerine dusunme gostergesi cevap diye donuyordu.
#[test]
fn dusunme_gostergesi_kararli_sayilmaz() {
    let mut d = BitisDedektoru::yeni(0);
    for _ in 0..10 {
        assert_eq!(
            d.gozlem(1, "Thinking... Skip"),
            None,
            "gosterge cevap sayilmamali"
        );
    }
    // Gercek cevap gelince normal kararlilik isler.
    let mut sonuc = None;
    // ESIK + 1: ilk gozlem `onceki`yi kurar, sayac ondan SONRA islemeye baslar.
    for _ in 0..=KARARLILIK_ESIGI {
        sonuc = d.gozlem(1, "Thought Process\nAnkara");
    }
    assert_eq!(
        sonuc.as_deref(),
        Some("Ankara"),
        "gurultu ayiklanmis cevap donmeli"
    );
}

/// Insan kapisi hatasi cagiran tarafta METINDEN taniniyor; onek degisirse
/// pool/server baglantisi sessizce kopar. Bu sinama o baglantinin bekcisi.
/// Surucu arizasi ORTAMSALDIR: havuz bunu hesaba ozel sanip sirayla butun
/// hesaplari deniyordu (olculdu: 300 sn, dort hesap bosuna cezalandirildi).
/// Ayrim metinden taniniyor; onek degisirse pool baglantisi sessizce kopar.
#[test]
fn surucu_arizasi_hesap_arizasindan_ayrilir() {
    let s =
        anyhow::anyhow!("{SURUCU_ARIZASI}: geckodriver'a baglanilamadi (http://127.0.0.1:5555)");
    assert!(format!("{s:#}").contains(SURUCU_ARIZASI));
    // Hesaba ozel arizalar bu sinifa DUSMEMELI, yoksa tek hesabin sorunu
    // butun istegi dusururdu.
    for h in ["oturum dusmus", "yanit gelmedi (120 sn)", "kota bildirdi"] {
        assert!(
            !anyhow::anyhow!("{h}").to_string().contains(SURUCU_ARIZASI),
            "{h}"
        );
    }
    // Insan kapisi ile de karismamali: biri insan bekler, oteki ortam onarimi.
    assert_ne!(SURUCU_ARIZASI, INSAN_KAPISI);
}

#[test]
fn insan_kapisi_oneki_taninabilir() {
    let h =
        anyhow::anyhow!("{INSAN_KAPISI}: dogrulama katmani acik (#aliyunCaptcha-mask.mask-show)");
    assert!(format!("{h:#}").contains(INSAN_KAPISI));
    // Siradan bir hata insan kapisi sayilmamali.
    let d = anyhow::anyhow!("yanit gelmedi (120 sn)");
    assert!(!format!("{d:#}").contains(INSAN_KAPISI));
}

#[test]
fn dogrulama_duvari_taninir() {
    assert!(dogrulama_duvari_mi(
        "No response, Please try again later.\nVerification failed, please try again"
    ));
    assert!(dogrulama_duvari_mi("Verification failed, please try again"));
    assert!(!dogrulama_duvari_mi("Ankara"));
    // Cevabin icinde gecen benzer bir cumle yanlis alarm vermemeli diye
    // kaliplar dar tutuldu; bu satir onun bekcisi.
    assert!(!dogrulama_duvari_mi("Dogrulama basarili, devam ediyorum"));
}

#[test]
fn uzunluk_tanisi_arizalari_ayirt_eder() {
    assert!(uzunluk_tanisi(40150, 32000).contains("KIRPMIS"));
    assert!(uzunluk_tanisi(40150, 40150).contains("ESIT"));
    assert!(uzunluk_tanisi(100, 0).contains("BOS"));
    assert!(uzunluk_tanisi(100, 101).contains("FAZLA"));
    assert!(uzunluk_tanisi(40150, 32000).contains("40150"));
}

/// Editorler satir sonu/bosluk ekleyebiliyor; bunlara takilip saglam bir yazimi
/// reddetmek de yanlis olurdu.
#[test]
fn bosluk_farki_yazimi_reddettirmez() {
    assert!(metin_esit_mi("  Merhaba   dunya \n", "Merhaba dunya"));
}

/// Olculdu: M365 Copilot'un editoru metnin sonuna U+200C birakiyor ve dogru
/// yazilmis istem "38 karakter, beklenen 37" diye reddediliyordu. Gorunmez
/// karakterler icerik tasimaz; ama EKSIK metin hala reddedilmeli.
#[test]
fn sifir_genislikli_karakter_yazimi_reddettirmez() {
    assert!(metin_esit_mi(
        "27 carpi 3 kactir?\u{200c}",
        "27 carpi 3 kactir?"
    ));
    assert!(metin_esit_mi(
        "\u{feff}Merhaba\u{200b} dunya",
        "Merhaba dunya"
    ));
    assert!(
        !metin_esit_mi("27 carpi\u{200c}", "27 carpi 3 kactir?"),
        "gorunmez karakter ayiklamak eksik metni gecirmemeli"
    );
}

/// Kilit tespiti dosyanin VARLIGINA degil, tutulup tutulmadigina bakmali:
/// cokme sonrasi kalan parent.lock yuzunden saglam bir profili "kilitli" sayip
/// hesabi kullanilamaz hale getirirdik.
#[test]
fn kilit_dosya_varligina_degil_tutuluyor_olmasina_bakar() {
    let dir = std::env::temp_dir().join(format!("kervan-kilit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lock = dir.join("parent.lock");

    // Kilit dosyasi yok
    std::fs::remove_file(&lock).ok();
    assert!(!profil_kilitli(&dir));

    // Dosya var ama tutan yok (cokmeden kalmis) — kilitli SAYILMAMALI
    std::fs::write(&lock, b"").unwrap();
    assert!(
        !profil_kilitli(&dir),
        "artik kilit dosyasi profili olu birakmamali"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// `-no-remote` iki baslatma yolunda da bulunmali.
///
/// ASIL HATA (olculdu 2026-10-21): otomasyon yolunda bu bayrak yoktu; Zen zaten
/// calisan ornege devrediyor, Marionette kapisi acilmiyor, `baglan` icindeki
/// `connect` sinirsiz bekliyordu — havuz istegi 240 sn+ sessizce asili kaliyor,
/// ne hata ne kayit dusuluyordu. Argumanlar `Command` cagrisina gomulu oldugu icin
/// hicbir test bunu goremiyordu; bu test o boslugu kapatir.
#[test]
fn iki_yol_da_no_remote_ve_profil_tasir() {
    let dir = Path::new("C:\\kervan\\profiles\\hesap-a");
    let otomasyon = otomasyon_arglari(dir);
    let elle = elle_giris_arglari(dir, "https://gemini.google.com/app");

    for (ad, args) in [("otomasyon", &otomasyon), ("elle giris", &elle)] {
        assert!(
            args.iter().any(|a| a == "-no-remote"),
            "{ad} yolunda -no-remote yok: Zen calisan ornege devreder ve Marionette acilmaz"
        );
        let i = args
            .iter()
            .position(|a| a == "-profile")
            .unwrap_or_else(|| panic!("{ad} yolunda -profile yok"));
        assert_eq!(
            args.get(i + 1).map(String::as_str),
            Some(dir.display().to_string().as_str()),
            "{ad} yolunda profil dizini yanlis"
        );
    }

    // Adres yalniz elle giris yolunda; otomasyon yoluna URL karismamali.
    assert_eq!(
        elle.last().map(String::as_str),
        Some("https://gemini.google.com/app")
    );
    assert!(!otomasyon.iter().any(|a| a.starts_with("http")));
}

/// ASIL HATA BUYDU: ayarlar yalnizca WebDriver yolunda uygulaniyordu. Hesap
/// kurarken kullanilan elle giris yolu tarayiciyi dogrudan baslattigi icin hicbir
/// ayar gitmiyor ve kurulum sihirbazi her seferinde cikiyordu. Iki yol ayni
/// listeyi tasimali; biri degisip oteki degismezse bu test dusmeli.
#[test]
fn iki_yol_ayni_ayarlari_tasir() {
    let caps = yetenekler(Path::new("z"), vec![]);
    let yetenek: std::collections::BTreeSet<String> = caps["moz:firefoxOptions"]["prefs"]
        .as_object()
        .expect("prefs nesnesi")
        .keys()
        .cloned()
        .collect();
    let dosya: std::collections::BTreeSet<String> = VARSAYILAN_USER_JS
        .lines()
        .filter_map(|l| l.strip_prefix("user_pref(\""))
        .filter_map(|l| l.split('"').next())
        .map(String::from)
        .collect();
    assert_eq!(
        dosya, yetenek,
        "elle giris ve WebDriver yollari ayni ayarlari vermeli"
    );
    assert!(dosya.contains("zen.welcome-screen.seen"));
}

/// Elle giris yolunda ayarlar profile DOSYA olarak yazilmali; WebDriver orada yok.
#[test]
fn tohumlama_profile_dosya_yazar() {
    let dir = std::env::temp_dir().join(format!("kervan-tohum2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Onceki kosudan kalmis yabanci icerik EZILMELI, yoksa ayarlar gitmez.
    std::fs::write(dir.join("user.js"), "user_pref(\"baska.sey\", 1);\n").unwrap();
    profil_tohumla(&dir).unwrap();
    let icerik = std::fs::read_to_string(dir.join("user.js")).unwrap();
    assert!(icerik.contains("zen.welcome-screen.seen"));
    assert!(!icerik.contains("baska.sey"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn yetenekler_karsilama_ayarlarini_tasir() {
    let caps = yetenekler(
        Path::new("C:\\zen.exe"),
        vec!["-profile".into(), "d".into()],
    );
    let prefs = &caps["moz:firefoxOptions"]["prefs"];
    assert_eq!(
        prefs["zen.welcome-screen.seen"], true,
        "Zen karsilama kapisi eksik"
    );
    assert_eq!(prefs["browser.sessionstore.resume_from_crash"], false);
    // profil bagimsizligi da ayni yetenekten geliyor; kirilirsa hesaplar birlesir
    assert_eq!(caps["moz:firefoxOptions"]["args"][0], "-profile");
}

/// Her oturum ayri port almali; sabit port ikinci hesabi engellerdi.
#[test]
fn portlar_benzersiz() {
    let a = bos_port().unwrap();
    let b = bos_port().unwrap();
    assert!(a > 0 && b > 0);
}

/// ASIL HATA BUYDU: ikinci turda onceki turun yaniti hala ekranda ve kararli.
/// Temel cizgi olmadan dedektor onu "tamamlanmis yeni yanit" sanip donduruyordu —
/// yani onceki sorunun cevabi, yeni sorunun cevabi diye. Sessiz ve yanlis.
#[test]
fn onceki_turun_yaniti_dondurulmez() {
    let mut d = BitisDedektoru::yeni(2); // gondermeden once 2 balon vardi
    for _ in 0..20 {
        // Sayi artmiyor: ekrandaki hala onceki turun yaniti.
        assert_eq!(d.gozlem(2, "onceki turun yaniti"), None);
    }
    assert_eq!(d.yarim(), "", "yeni balon yokken metin biriktirilmemeli");
}

/// Yeni balon gelip buyumesi durunca yanit donmeli.
#[test]
fn yeni_yanit_kararlilasinca_donulur() {
    let mut d = BitisDedektoru::yeni(2);
    assert_eq!(d.gozlem(3, "el"), None);
    assert_eq!(d.gozlem(3, "elma"), None);
    // ayni metin ust uste: esige kadar None, esikte deger
    for _ in 0..KARARLILIK_ESIGI - 1 {
        assert_eq!(d.gozlem(3, "elma"), None);
    }
    assert_eq!(d.gozlem(3, "elma"), Some("elma".to_string()));
}

/// Balon acildi ama henuz bos: bekleme surmeli, bos yanit dondurulmemeli.
#[test]
fn bos_balon_bitis_sayilmaz() {
    let mut d = BitisDedektoru::yeni(0);
    for _ in 0..10 {
        assert_eq!(d.gozlem(1, "   "), None);
    }
}

/// Yanit buyurken kesilirse elde kalan YARIM metin saklanmali; `ask` bunu
/// tam yanit gibi degil, yarim oldugunu soyleyerek bildiriyor.
#[test]
fn yarim_metin_saklanir() {
    let mut d = BitisDedektoru::yeni(0);
    d.gozlem(1, "yarim kalmis ya");
    assert_eq!(d.yarim(), "yarim kalmis ya");
}

/// Bulunamayan tarayici sessizce baska bir seye dusmemeli.
#[test]
fn eksik_tarayici_acik_hata_verir() {
    unsafe { std::env::set_var("KERVAN_BROWSER", "C:\\olmayan\\tarayici.exe") };
    let r = BrowserBinary::discover();
    unsafe { std::env::remove_var("KERVAN_BROWSER") };
    assert!(r.is_err(), "olmayan yol icin hata bekleniyordu");
}
