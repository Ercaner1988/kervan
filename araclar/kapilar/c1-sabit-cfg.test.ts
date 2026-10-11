// sabit-yanit: parametreye ya da yapı alanına konan #[cfg(test)] yalnız o parametreyi/alanı kapsar;
// ardından gelen üretim kodunu taramadan düşürmemeli (yargıç engeli, C1 3. tur).
import { afterAll, describe, expect, it } from "bun:test";
import fs from "fs";
import os from "os";
import path from "path";
import { dosyayiTara } from "./sabit-yanit.ts";

const KOK = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-sabit-cfg-"));
afterAll(() => fs.rmSync(KOK, { recursive: true, force: true }));
const tara = (ad: string, kaynak: string) => {
  const y = path.join(KOK, ad + ".rs");
  fs.writeFileSync(y, kaynak);
  return dosyayiTara(y).map((b) => b.satirNo);
};
const SAHTE = 'pub fn sohbet() -> &\'static str { "chatcmpl-123" }\n';

describe("c1::sabit_yanit_cfg_kapsami", () => {
  it("c1::parametre_ozniteligi_sonraki_uretim_islevini_gizlemez", () => {
    expect(tara("param", "fn yardimci(#[cfg(test)] _a: u8) {}\n" + SAHTE)).toEqual([2]);
  });
  it("c1::yapi_alani_ozniteligi_sonraki_uretim_islevini_gizlemez", () => {
    expect(tara("alan", "struct S { #[cfg(test)] a: u8 }\n" + SAHTE)).toEqual([2]);
  });
  it("c1::pub_yapi_alani_ozniteligi_sonraki_uretim_islevini_gizlemez", () => {
    expect(tara("pubalan", "pub struct S { #[cfg(test)] pub a: u8, b: u8 }\n" + SAHTE)).toEqual([2]);
  });
  it("c1::enum_varyanti_ozniteligi_sonraki_uretim_islevini_gizlemez", () => {
    expect(tara("varyant", "enum E { #[cfg(test)] A, B }\n" + SAHTE)).toEqual([2]);
  });
  it("c1::test_modulu_hala_gizlenir", () => {
    expect(tara("mod", '#[cfg(test)]\nmod tests { fn t() { let _ = "chatcmpl-123"; } }\n')).toEqual([]);
  });
  it("c1::jenerik_test_impl_hala_gizlenir", () => {
    expect(tara("impl", '#[cfg(test)]\nimpl<A, B> X<A, B> { fn t() -> &\'static str { "chatcmpl-123" } }\n')).toEqual([]);
  });
});
