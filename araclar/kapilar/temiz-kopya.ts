import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-1.2: Temiz Kopya Doğrulayıcısı (temiz-kopya.ts)
// Belirtilen commit'i geçici bir git worktree'ye çıkarıp komutu orada çalıştırır.
// Çalışma dizinine ASLA dokunmaz. İşlem sonrası geçici worktree'yi temizler.

export function gitDurumunuAl(dizin = process.cwd()): string {
  const proc = spawnSync("git", ["status", "--porcelain"], {
    cwd: dizin,
    encoding: "utf-8",
  });
  return proc.stdout ? proc.stdout.trim() : "";
}

export function temizKopyadaKos(
  commit = "HEAD",
  komut = "bun araclar/kapilar/hepsi.ts",
  geciciKokDizin = path.join("port", "kayit", "_gecici")
): {
  cikis: number;
  stdout: string;
  stderr: string;
  calismaDiziniDegistiMi: boolean;
  geciciWorktreeYolu: string;
} {
  const baslangicDurumu = gitDurumunuAl();

  if (!fs.existsSync(geciciKokDizin)) {
    fs.mkdirSync(geciciKokDizin, { recursive: true });
  }

  const benzersizAd = `wt_${Date.now()}_${Math.random().toString(36).substring(2, 8)}`;
  const geciciYol = path.resolve(geciciKokDizin, benzersizAd);

  // 1. Git worktree oluştur
  const addProc = spawnSync("git", ["worktree", "add", "--detach", geciciYol, commit], {
    encoding: "utf-8",
  });

  if (addProc.status !== 0) {
    return {
      cikis: addProc.status ?? 1,
      stdout: addProc.stdout || "",
      stderr: `git worktree add başarısız: ${addProc.stderr || ""}`,
      calismaDiziniDegistiMi: false,
      geciciWorktreeYolu: geciciYol,
    };
  }

  // 2. Komutu geçici kopyada çalıştır
  const kosProc = spawnSync(komut, {
    cwd: geciciYol,
    shell: true,
    encoding: "utf-8",
    env: { ...process.env },
  });

  const cikis = kosProc.status ?? 1;
  const stdout = kosProc.stdout || "";
  const stderr = kosProc.stderr || "";

  // 3. Geçici worktree'yi kaldır ve sil
  spawnSync("git", ["worktree", "remove", "--force", geciciYol], {
    encoding: "utf-8",
  });

  if (fs.existsSync(geciciYol)) {
    try {
      fs.rmSync(geciciYol, { recursive: true, force: true });
    } catch {}
  }

  // 4. Çalışma dizini durumunu karşılaştır
  const bitisDurumu = gitDurumunuAl();
  const calismaDiziniDegistiMi = baslangicDurumu !== bitisDurumu;

  return {
    cikis,
    stdout,
    stderr,
    calismaDiziniDegistiMi,
    geciciWorktreeYolu: geciciYol,
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let commit = "HEAD";
  let komut = "bun araclar/kapilar/hepsi.ts";

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--komut" && i + 1 < args.length) {
      komut = args[++i];
    } else if (!args[i].startsWith("--")) {
      commit = args[i];
    }
  }

  console.log(`[temiz-kopya] Commit '${commit}' geçici worktree'ye çıkarılıyor...`);
  console.log(`[temiz-kopya] Çalıştırılacak komut: ${komut}`);

  const sonuc = temizKopyadaKos(commit, komut);

  if (sonuc.calismaDiziniDegistiMi) {
    console.error(`❌ [temiz-kopya] HATA: Çalışma dizini durumu bozuldu!`);
    process.exit(2);
  }

  console.log(`[temiz-kopya] Temiz kopya çıkış kodu: ${sonuc.cikis}`);
  if (sonuc.stdout) console.log(sonuc.stdout);
  if (sonuc.stderr) console.error(sonuc.stderr);

  process.exit(sonuc.cikis);
}
