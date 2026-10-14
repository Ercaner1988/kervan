**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# كرفان

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

بنية بوابة توجه استخدام الذكاء الاصطناعي من واجهات الويب إلى نقاط النهاية المطلوبة. Rust، ملف ثنائي واحد.

> حملتُ على دروب كثيرة، وأعلمُ ما يتحمّلُ كلُّ طريق.

---

## 📦 المتطلبات

| المتطلب | الإصدار / ملاحظة |
|---|---|
| Rust | 1.70+ (للبناء) |
| Firefox | Zen أو Firefox القياسي (مع geckodriver) |
| نظام التشغيل | Windows، Linux، macOS |

## 🛠 الوظائف والميزات

يحوّل حسابات الذكاء الاصطناعي بدون مفاتيح API (واجهات Gemini و Claude و Perplexity) إلى نقطة نهاية واحدة متوافقة مع OpenAI. بدلاً من نسخ ملفات تعريف الارتباط، يشغّل **متصفحاً حقيقياً**: تسجيل الدخول مرة واحدة، والجلسة تبقى في دليل الملف الشخصي.

الفرق عن الحلول الموجودة في ثلاث نقاط:

- **عزل حقيقي لكل حساب** — كل حساب في ملفه الشخصي الخاص. وضع التصفح الخاص لا يكفي: كل نوافذ التصفح الخاص في نفس المتصفح تتشارك ملفاً شخصياً واحداً.
- **جلسة دائمة** — لا لصق لملفات تعريف الارتباط، لا تجديد عند دورانها.
- **بوابة بشرية** — لا يفشل بصمت عند جدار التحقق؛ يوقف الحساب، يجلب النافذة إلى المقدمة، ويتركك تحلّه. لا يحاول تجاوز كشف الروبوتات.

## ⚙ التثبيت

### خطوات التثبيت

1. استنسخ المستودع:

```bash
git clone https://github.com/Ercaner1988/kervan.git
cd kervan
```

2. ابنِ المشروع:

```bash
cargo build --release
```

3. أنشئ ملف التكوين:

```bash
cp kervan.toml.example kervan.toml
```

4. عرّف حساباتك في `kervan.toml`.

### الاستخدام

```bash
./target/release/kervan
```

يعمل الخادم على `127.0.0.1:20129`. نقطة نهاية متوافقة مع OpenAI:

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "مرحباً"}]}'
```

## 🗺 الحالة وخارطة الطريق

| الحالة | الموضوع |
|---|---|
| ✅ جاهز | عدة مزودين، عدة حسابات، ميزانية السرعة، بوابة بشرية |
| 🔬 تجريبي | تعلم الحصة (حدود الحصة المرصودة) |
| 📅 مستقبلاً | قوالب `kervan.toml`، تعلم الحصة لكل مزود |

### التفاصيل التقنية

| | |
|---|---|
| المتصفح | مبني على Firefox (Zen أو Firefox)، عبر geckodriver |
| نقطة النهاية | `POST /v1/chat/completions`، `GET /v1/models`، `GET /health` |
| المجموعة | `GET /accounts`، `POST /accounts/{i}/devam` |
| الربط | المضيف المحلي فقط (`127.0.0.1`) |

### المزودون

معرّفون: `gemini`، `kimi`، `copilot`، `copilot-m365`، `perplexity`، `claude`، `deepseek`، `qwen`، `z.ai`. كل حساب مرتبط بمزود واحد أو أكثر في `kervan.toml`.

#### حالة المزودين

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-14T05:51:04.955Z)
- Tüm kapılar yeşil mi: hayır (9/11)
- Fazlar: yok
- Defter: planlandi 5, dogrulandi 1; route_ts 730
<!-- DURUM:BITIS -->

##### ملاحظات المزودين (مكتوبة يدويًا؛ لا تتحقق منها البوابة)

| المزود | الحالة |
|---|---|
| `gemini` | ✅ في المجموعة — عدة حسابات، ~11 ثانية |
| `kimi` | ✅ في المجموعة — عدة حسابات، 25–36 ثانية |
| `copilot` | ✅ في المجموعة — حساب Microsoft شخصي، ~12 ثانية |
| `copilot-m365` | ✅ في المجموعة — حساب عمل/مدرسة، ~14 ثانية |
| `perplexity` | ⚠️ غير مستقر — بعض الحسابات تواجه Cloudflare |
| `claude` | ❌ Cloudflare — معطل في المجموعة |
| `deepseek` | ❌ Cloudflare — معظم الحسابات |
| `qwen` | ❌ Cloudflare — جميع الحسابات |
| `z.ai` | ❌ Cloudflare — ردود فارغة مضللة |

## ⚠️ المشاكل المعروفة / الميزات التجريبية

- **تعلم الحصة:** مزودو الويب لا يكشفون حالة الحصة. كرفان يسجل نتيجة كل طلب في `%LOCALAPPDATA%\kervan\kota.jsonl` ويحدد نفاد الحصة.
- **Cloudflare:** Claude و Deepseek و Qwen و Z.ai تواجه جدران Cloudflare في معظم الحسابات.
- **البوابة البشرية:** عند جدار التحقق، يوقف الحساب ويجلب النافذة إلى المقدمة — لا يوجد حل آلي.

## 🤝 المساهمون

- [إركان إير](https://github.com/Ercaner1988) — التصميم والتطوير

## 📜 الترخيص

MIT — انظر [LICENSE](LICENSE).