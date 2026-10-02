//! SQLite Göç Yürütücüsü ve Şema Denetleyicisi
//!
//! OmniRoute'un 193 SQL göçünü sırayla okur, uygular ve altın şemayla doğrular.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Göç dosyalarını tespit edip sıralı olarak okur.
pub fn goc_dosyalarini_yukle(goc_dizini: &Path) -> Result<Vec<(String, String)>> {
    if !goc_dizini.exists() {
        anyhow::bail!("Göç dizini bulunamadı: {}", goc_dizini.display());
    }

    let mut dosyalar: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(goc_dizini)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("sql") {
            dosyalar.push(path);
        }
    }

    // Dosya adına göre sırala (001_..., 002_... 193_...)
    dosyalar.sort();

    let mut sonuc = Vec::new();
    for yol in dosyalar {
        let dosya_adi = yol
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("bilinmeyen.sql")
            .to_string();
        let icerik = fs::read_to_string(&yol)
            .with_context(|| format!("Göç dosyası okunamadı: {}", yol.display()))?;
        sonuc.push((dosya_adi, icerik));
    }

    Ok(sonuc)
}

/// Veritabanındaki aktif tabloların isimlerini listeler.
pub fn tablolari_listele(conn: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut tablolar = Vec::new();
    for r in rows {
        tablolar.push(r?);
    }
    Ok(tablolar)
}

/// Bir tablonun veya görünümün var olup olmadığını denetler.
pub fn tablo_var_mi(conn: &rusqlite::Connection, tablo_adi: &str) -> bool {
    let mut stmt = match conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1")
    {
        Ok(s) => s,
        Err(_) => return false,
    };
    stmt.exists([tablo_adi]).unwrap_or(false)
}

/// Bir tabloda belirli bir sütunun var olup olmadığını denetler.
pub fn sutun_var_mi(conn: &rusqlite::Connection, tablo_adi: &str, sutun_adi: &str) -> bool {
    let pragma_sql = format!("PRAGMA table_info({})", tablo_adi);
    let mut stmt = match conn.prepare(&pragma_sql) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let rows = match stmt.query_map([], |r| r.get::<_, String>(1)) {
        Ok(r) => r,
        Err(_) => return false,
    };
    for col in rows.flatten() {
        if col == sutun_adi {
            return true;
        }
    }
    false
}

fn sutun_ekle_guvenli(conn: &rusqlite::Connection, sql: &str) -> Result<()> {
    if let Err(err) = conn.execute_batch(sql) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err).context("Sütun ekleme başarısız oldu");
        }
    }
    Ok(())
}

/// OmniRoute schemaColumns.ts (ensure*Columns) ve apiKeyColumnFallbacks.ts şema iyileştiricisi:
/// Eski SQL şablonlarının ve yeni göçlerin uyumunu sağlayan dinamik sütun tamamlayıcı.
pub fn sutunlari_guvenceye_al(conn: &rusqlite::Connection) -> Result<()> {
    // 1. usage_history sütunları (OmniRoute ensureUsageHistoryColumns)
    if tablo_var_mi(conn, "usage_history") {
        let uh_sutunlar = [
            ("success", "INTEGER DEFAULT 1"),
            ("latency_ms", "INTEGER DEFAULT 0"),
            ("ttft_ms", "INTEGER DEFAULT 0"),
            ("error_code", "TEXT"),
            ("service_tier", "TEXT DEFAULT 'standard'"),
            ("combo_strategy", "TEXT DEFAULT 'direct'"),
            ("account_key", "TEXT"),
            ("account_label", "TEXT"),
            ("account_label_priority", "INTEGER DEFAULT 0"),
        ];
        for (col, col_def) in uh_sutunlar {
            if !sutun_var_mi(conn, "usage_history", col) {
                let sql = format!("ALTER TABLE usage_history ADD COLUMN {} {};", col, col_def);
                sutun_ekle_guvenli(conn, &sql)?;
            }
        }
    }

    // 2. provider_connections sütunları (OmniRoute ensureProviderConnectionsColumns)
    if tablo_var_mi(conn, "provider_connections") {
        let pc_sutunlar = [
            ("auth_type", "TEXT"),
            ("name", "TEXT"),
            ("email", "TEXT"),
            ("display_name", "TEXT"),
            ("provider_specific_data", "TEXT"),
            ("rate_limit_protection", "INTEGER DEFAULT 0"),
            ("last_used_at", "TEXT"),
            ("default_model", "TEXT"),
            ("last_ping_at", "TEXT"),
            ("last_pinged_reset_key", "TEXT"),
            ("group", "TEXT"),
            ("max_concurrent", "INTEGER"),
            ("proxy_enabled", "INTEGER NOT NULL DEFAULT 1"),
            ("per_key_proxy_enabled", "INTEGER NOT NULL DEFAULT 0"),
            ("quota_visible", "INTEGER NOT NULL DEFAULT 1"),
        ];
        for (col, col_def) in pc_sutunlar {
            if !sutun_var_mi(conn, "provider_connections", col) {
                let sql = if col == "group" {
                    format!(
                        "ALTER TABLE provider_connections ADD COLUMN \"group\" {};",
                        col_def
                    )
                } else {
                    format!(
                        "ALTER TABLE provider_connections ADD COLUMN {} {};",
                        col, col_def
                    )
                };
                sutun_ekle_guvenli(conn, &sql)?;
            }
        }
    }

    // 3. proxy_logs sütunları (OmniRoute ensureProxyLogsColumns)
    if tablo_var_mi(conn, "proxy_logs") {
        let pl_sutunlar = [
            ("egress_ip", "TEXT"),
            ("upstream_status", "INTEGER"),
            ("proxy_name", "TEXT"),
            ("rotation_account", "TEXT"),
            ("correlation_id", "TEXT"),
            ("attempt_number", "INTEGER"),
            ("attempt_issue", "TEXT"),
            ("headers_ms", "INTEGER DEFAULT NULL"),
            ("first_chunk_ms", "INTEGER DEFAULT NULL"),
        ];
        for (col, col_def) in pl_sutunlar {
            if !sutun_var_mi(conn, "proxy_logs", col) {
                let sql = format!("ALTER TABLE proxy_logs ADD COLUMN {} {};", col, col_def);
                sutun_ekle_guvenli(conn, &sql)?;
            }
        }
    }

    // 4. api_keys sütunları (OmniRoute API_KEY_COLUMN_FALLBACKS)
    if tablo_var_mi(conn, "api_keys") {
        let ak_sutunlar = [
            ("allowed_models", "TEXT"),
            (
                "model_access_mode",
                "TEXT NOT NULL DEFAULT 'all' CHECK (model_access_mode IN ('all', 'restricted'))",
            ),
            ("blocked_models", "TEXT"),
            ("allowed_combos", "TEXT"),
            ("no_log", "INTEGER NOT NULL DEFAULT 0"),
            ("allowed_connections", "TEXT"),
            ("auto_resolve", "INTEGER NOT NULL DEFAULT 0"),
            ("is_active", "INTEGER NOT NULL DEFAULT 1"),
            ("access_schedule", "TEXT"),
            ("max_requests_per_day", "INTEGER"),
            ("max_requests_per_minute", "INTEGER"),
            ("throttle_delay_ms", "INTEGER"),
            ("max_sessions", "INTEGER NOT NULL DEFAULT 0"),
            ("revoked_at", "TEXT"),
            ("expires_at", "TEXT"),
            ("last_used_at", "TEXT"),
            ("key_prefix", "TEXT"),
            ("ip_allowlist", "TEXT"),
            ("scopes", "TEXT"),
            ("rate_limits", "TEXT"),
            ("is_banned", "INTEGER NOT NULL DEFAULT 0"),
            ("key_hash", "TEXT"),
            ("proxy_id", "TEXT"),
            ("allowed_endpoints", "TEXT"),
            ("allowed_quotas", "TEXT NOT NULL DEFAULT '[]'"),
            ("stream_default_mode", "TEXT NOT NULL DEFAULT 'legacy'"),
            ("cache_default_mode", "TEXT NOT NULL DEFAULT 'legacy'"),
            ("disable_non_public_models", "INTEGER NOT NULL DEFAULT 0"),
            ("allow_usage_command", "INTEGER NOT NULL DEFAULT 0"),
            ("usage_limit_enabled", "INTEGER NOT NULL DEFAULT 0"),
            ("daily_usage_limit_usd", "REAL"),
            ("weekly_usage_limit_usd", "REAL"),
            ("chaos_mode_enabled", "INTEGER NOT NULL DEFAULT 0"),
            ("compression_enabled", "INTEGER NOT NULL DEFAULT 1"),
            ("allow_auto_combos", "INTEGER NOT NULL DEFAULT 1"),
            (
                "catalog_scope",
                "TEXT NOT NULL DEFAULT 'all' CHECK (catalog_scope IN ('all', 'combos', 'models'))",
            ),
        ];
        for (col, col_def) in ak_sutunlar {
            if !sutun_var_mi(conn, "api_keys", col) {
                let sql = format!("ALTER TABLE api_keys ADD COLUMN {} {};", col, col_def);
                sutun_ekle_guvenli(conn, &sql)?;
            }
        }
    }

    Ok(())
}

/// OmniRoute'un migrationRunner.ts dosyasındaki `isSchemaAlreadyApplied` fonksiyonunun
/// birebir kural eşleştirmesi. İlk şema zaten bu sütun/tabloları içeriyorsa ilgili göç
/// atlanır (idempotency).
pub fn sema_zaten_uygulanmis_mi(conn: &rusqlite::Connection, dosya_adi: &str) -> bool {
    let versiyon = match dosya_adi.split('_').next() {
        Some(v) => v,
        None => return false,
    };

    match versiyon {
        "003" => sutun_var_mi(conn, "provider_nodes", "chat_path"),
        "005" => sutun_var_mi(conn, "combos", "system_message"),
        "007" => sutun_var_mi(conn, "call_logs", "request_type"),
        "009" => sutun_var_mi(conn, "call_logs", "requested_model"),
        "018" => {
            sutun_var_mi(conn, "call_logs", "tokens_cache_read")
                && sutun_var_mi(conn, "call_logs", "tokens_cache_creation")
                && sutun_var_mi(conn, "call_logs", "tokens_reasoning")
        }
        "020" => sutun_var_mi(conn, "combos", "sort_order"),
        "021" => {
            sutun_var_mi(conn, "call_logs", "combo_step_id")
                && sutun_var_mi(conn, "call_logs", "combo_execution_key")
        }
        "023" => sutun_var_mi(conn, "memories", "memory_id"),
        "025" => {
            sutun_var_mi(conn, "call_logs", "detail_state")
                && sutun_var_mi(conn, "call_logs", "request_summary")
        }
        "026" => sutun_var_mi(conn, "call_logs", "cache_source"),
        "027" => sutun_var_mi(conn, "skills", "mode"),
        "028" => tablo_var_mi(conn, "batches") && tablo_var_mi(conn, "files"),
        "029" => sutun_var_mi(conn, "provider_connections", "max_concurrent"),
        "040" => sutun_var_mi(conn, "proxy_registry", "source"),
        "041" => {
            if dosya_adi.contains("session_account_affinity") {
                tablo_var_mi(conn, "session_account_affinity")
            } else {
                sutun_var_mi(conn, "compression_analytics", "actual_prompt_tokens")
                    && sutun_var_mi(conn, "compression_analytics", "actual_completion_tokens")
                    && sutun_var_mi(conn, "compression_analytics", "actual_total_tokens")
                    && sutun_var_mi(conn, "compression_analytics", "receipt_source")
                    && sutun_var_mi(conn, "compression_analytics", "validation_fallback")
                    && sutun_var_mi(conn, "compression_analytics", "output_mode")
            }
        }
        "042" => {
            tablo_var_mi(conn, "compression_combos")
                && tablo_var_mi(conn, "compression_combo_assignments")
                && sutun_var_mi(conn, "compression_analytics", "compression_combo_id")
                && sutun_var_mi(conn, "compression_analytics", "engine")
        }
        "045" => sutun_var_mi(conn, "call_logs", "tokens_compressed"),
        "053" => !sutun_var_mi(conn, "files", "status"),
        "054" => sutun_var_mi(conn, "usage_history", "service_tier"),
        "062" => sutun_var_mi(conn, "usage_history", "combo_strategy"),
        "070" => {
            sutun_var_mi(conn, "webhooks", "kind")
                && sutun_var_mi(conn, "webhooks", "metadata_encrypted")
        }
        "071" => {
            sutun_var_mi(conn, "version_manager", "logs_buffer_path")
                && sutun_var_mi(conn, "version_manager", "provider_expose")
                && sutun_var_mi(conn, "version_manager", "last_sync_at")
        }
        "073" => sutun_var_mi(conn, "memories", "needs_reindex"),
        "085" => tablo_var_mi(conn, "quota_pools") && tablo_var_mi(conn, "quota_allocations"),
        "088" => {
            tablo_var_mi(conn, "quota_groups") && sutun_var_mi(conn, "quota_pools", "group_id")
        }
        "089" => sutun_var_mi(conn, "api_keys", "disable_non_public_models"),
        "090" => tablo_var_mi(conn, "plugin_metrics"),
        "091" => tablo_var_mi(conn, "plugin_analytics"),
        "095" => sutun_var_mi(conn, "provider_nodes", "custom_headers_json"),
        "117" => sutun_var_mi(conn, "proxy_assignments", "position"),
        "137" => sutun_var_mi(conn, "version_manager", "auto_restart_adopted"),
        "138" => sutun_var_mi(conn, "upstream_proxy_config", "fallback_backend"),
        "139" => tablo_var_mi(conn, "ccr_blocks"),
        "140" => tablo_var_mi(conn, "connection_runtime_state"),
        "143" => sutun_var_mi(conn, "api_keys", "cache_default_mode"),
        "153" => tablo_var_mi(conn, "radar_local_model_state"),
        "172" => {
            sutun_var_mi(conn, "provider_nodes", "daily_quota_reset_timezone")
                && sutun_var_mi(conn, "provider_nodes", "daily_quota_reset_hour")
        }
        "179" => sutun_var_mi(conn, "proxy_logs", "upstream_status"),
        "181" => sutun_var_mi(conn, "proxy_logs", "proxy_name"),
        "183" => sutun_var_mi(conn, "proxy_logs", "rotation_account"),
        "184" => sutun_var_mi(conn, "proxy_logs", "correlation_id"),
        "187" => sutun_var_mi(conn, "proxy_logs", "attempt_number"),
        "194" => sutun_var_mi(conn, "proxy_logs", "headers_ms"),
        _ => false,
    }
}
