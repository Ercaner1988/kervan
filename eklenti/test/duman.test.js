// İlk duman testi: sandbox gerçekten content_common.js'i yüklüyor mu,
// öğe okuma ve işaret ölçümü çalışıyor mu?
import { test, expect } from "bun:test";
import { sayfaKur, satirla } from "./harness.js";

test("sandbox content_common'i yükler ve ham liste okur", () => {
  const { window, document } = sayfaKur(
    `<button id="m">Model</button>
     <div id="menu">
       <div role="menuitem" class="mode selected"><span class="label">3.6 Flash</span></div>
       <div role="menuitem"><span class="label">3.1 Pro</span></div>
     </div>`
  );
  expect(typeof window.kervanOgeOkuHam).toBe("function");
  const liste = window.kervanOgeOkuHam("[role=menuitem]");
  expect(liste.length).toBe(2);
  expect(liste[0].metin).toBe("3.6 Flash");
  expect(liste[0].isaretli).toBe(true);
  expect(liste[1].isaretli).toBe(false);
});
