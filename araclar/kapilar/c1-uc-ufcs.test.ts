// uc-kaydi rota taraması yalnız `.route(` yöntem çağrısını görüyordu; aynı axum API'sinin UFCS biçimi
// `Router::route(r, "/gizli", post(h))` ne rota ne hata sayılıyordu (yargıç engeli, C1 3. tur). Fail-closed:
// çözülemeyen biçim hatadır.
import { afterAll, describe, expect, it } from "bun:test";
import fs from "fs";
import os from "os";
import path from "path";
import { rotaTaramasi } from "./uc-kaydi.ts";

const KOK = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-uc-ufcs-"));
afterAll(() => fs.rmSync(KOK, { recursive: true, force: true }));
const agac = (ad: string, kaynak: string) => {
  const d = path.join(KOK, ad);
  fs.mkdirSync(path.join(d, "crates", "a", "src"), { recursive: true });
  fs.writeFileSync(path.join(d, "crates", "a", "src", "lib.rs"), kaynak);
  return d;
};

describe("c1::uc_kaydi_ufcs", () => {
  it("c1::ufcs_router_route_hata_sayilir", () => {
    const r = rotaTaramasi(agac("ufcs", 'pub fn r() -> axum::Router { let r = axum::Router::new(); Router::route(r, "/gizli", post(h)) }\n'));
    expect(r.hatalar.join("\n")).toContain("route");
  });
  it("c1::turbofisli_ufcs_router_route_hata_sayilir", () => {
    const r = rotaTaramasi(agac("turbofis", 'pub fn r() { Router::<()>::route(Router::new(), "/gizli2", get(h)); }\n'));
    expect(r.hatalar.join("\n")).toContain("route");
  });
  it("c1::yontem_cagrisi_route_rota_sayilir_hata_vermez", () => {
    const r = rotaTaramasi(agac("yontem", 'pub fn r() { Router::new().route("/acik", get(h)); }\n'));
    expect(r.hatalar).toEqual([]);
    expect(r.rotalar.map((x) => x.yol)).toEqual(["/acik"]);
  });
  it("c1::router_disi_merge_yanlis_alarm_vermez", () => {
    const r = rotaTaramasi(agac("merge", 'pub fn r(a: Vec<u8>) { let _x = Kume::merge(a); }\n'));
    expect(r.hatalar).toEqual([]);
  });
});
