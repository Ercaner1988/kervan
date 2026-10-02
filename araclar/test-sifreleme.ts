/**
 * Şifreleme Test Vektörü Üretici (araclar/test-sifreleme.ts)
 */

import { encrypt, decrypt } from "../ref/omniroute-dbe703a/src/lib/db/encryption.ts";

process.env.STORAGE_ENCRYPTION_KEY = "test-secret-key-32-bytes-long-kervan!";

const plaintext = "sk-ant-api03-kervan-ve-omniroute-parite-testi-12345";
const encrypted = encrypt(plaintext);
console.log("Plaintext :", plaintext);
console.log("Encrypted :", encrypted);

const decrypted = decrypt(encrypted);
console.log("Decrypted :", decrypted);

if (decrypted === plaintext) {
  console.log("✅ Node/Bun tarafında şifreleme/çözümleme başarılı.");
} else {
  console.error("❌ Eşleşmedi!");
}
