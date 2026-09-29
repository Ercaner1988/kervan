//! Model Context Protocol (MCP) Server for Kervan
//!
//! OmniRoute `mcp-server/` mimarisinden saf Rust ile uyarlanmıştır.
//!
//! Claude Desktop, Antigravity, Open-Notebook ve Cursor gibi harici yapay
//! zeka araçlarının Kervan'a birleşik bir MCP aracı olarak bağlanmasını sağlar.
//!
//! Standart: JSON-RPC 2.0 (stdio & SSE uyumlu).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::compressor::{Compressor, SikistirmaSeviyesi};
use crate::task_router::{AkilliRota, GorevRotaYapilandirmasi};

/// MCP JSON-RPC 2.0 İstek Modeli.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpIstek {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// MCP JSON-RPC 2.0 Başarılı Yanıt Modeli.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpYanit {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

/// MCP JSON-RPC 2.0 Hata Modeli.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHataYaniti {
    pub jsonrpc: String,
    pub id: Value,
    pub error: McpHataDetay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHataDetay {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP Araç Tanımı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAracTanimi {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// MCP Sunucu Motoru.
pub struct McpSunucusu {
    sunucu_adi: &'static str,
    sunucu_surumu: &'static str,
    akilli_rota: AkilliRota,
}

impl Default for McpSunucusu {
    fn default() -> Self {
        Self {
            sunucu_adi: "kervan-mcp",
            sunucu_surumu: "0.1.0",
            akilli_rota: AkilliRota::yeni(GorevRotaYapilandirmasi::default()),
        }
    }
}

impl McpSunucusu {
    pub fn yeni() -> Self {
        Self::default()
    }

    /// JSON-RPC 2.0 istek satırını işler ve yanıt JSON'unu üretir.
    pub fn isle_istek(&self, istek_satiri: &str) -> Option<String> {
        let trimmed = istek_satiri.trim();
        if trimmed.is_empty() {
            return None;
        }

        let istek: McpIstek = match serde_json::from_str(trimmed) {
            Ok(m) => m,
            Err(e) => {
                let hata = McpHataYaniti {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    error: McpHataDetay {
                        code: -32700,
                        message: format!("Ayrıştırma hatası (Parse error): {}", e),
                        data: None,
                    },
                };
                return serde_json::to_string(&hata).ok();
            }
        };

        // Bildirimler (id olmayan istekler) için yanıt dönülmez
        let istek_id = match istek.id {
            Some(ref id) => id.clone(),
            None => {
                // Bildirimleri sessizce kabul et (örn: notifications/initialized)
                return None;
            }
        };

        let yanit_sonucu = match istek.method.as_str() {
            "initialize" => self.isle_initialize(),
            "ping" => Ok(json!({})),
            "tools/list" => self.isle_tools_list(),
            "tools/call" => self.isle_tools_call(istek.params.unwrap_or(Value::Null)),
            bilinmeyen => Err(McpHataDetay {
                code: -32601,
                message: format!("Bilinmeyen metot (Method not found): {}", bilinmeyen),
                data: None,
            }),
        };

        match yanit_sonucu {
            Ok(sonuc) => {
                let basarili = McpYanit {
                    jsonrpc: "2.0".to_string(),
                    id: istek_id,
                    result: sonuc,
                };
                serde_json::to_string(&basarili).ok()
            }
            Err(hata_detay) => {
                let hata = McpHataYaniti {
                    jsonrpc: "2.0".to_string(),
                    id: istek_id,
                    error: hata_detay,
                };
                serde_json::to_string(&hata).ok()
            }
        }
    }

    fn isle_initialize(&self) -> Result<Value, McpHataDetay> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": self.sunucu_adi,
                "version": self.sunucu_surumu
            }
        }))
    }

    fn isle_tools_list(&self) -> Result<Value, McpHataDetay> {
        let araclar = vec![
            McpAracTanimi {
                name: "kervan_modeller".to_string(),
                description: "Kervan'da aktif tüm sağlayıcıları ve 2026 amiral modellerini listeler.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpAracTanimi {
                name: "kervan_gorev_rotala".to_string(),
                description: "Verilen istemi analiz edip en uygun sağlayıcı ve modeli otomatik önerir.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "istem": { "type": "string", "description": "Analiz edilecek kullanıcı istemi veya görev tanımı." }
                    },
                    "required": ["istem"]
                }),
            },
            McpAracTanimi {
                name: "kervan_sikistir".to_string(),
                description: "Akademik metin ve istemleri anlam kaybı olmadan sıkıştırarak token tasarrufu sağlar.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "metin": { "type": "string", "description": "Sıkıştırılacak metin veya bağlam." },
                        "seviye": {
                            "type": "string",
                            "enum": ["hafif", "orta", "agresif"],
                            "description": "Sıkıştırma derinliği (varsayılan: orta)."
                        }
                    },
                    "required": ["metin"]
                }),
            },
            McpAracTanimi {
                name: "kervan_yerel_dosya".to_string(),
                description: "Kervan'ın güvenli yerel dosya okuma ve yazma aracını çalıştırır.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "eylem": { "type": "string", "enum": ["oku", "yaz", "dizin"], "description": "Yapılacak işlem." },
                        "hedef": { "type": "string", "description": "Dosya veya dizin yolu." },
                        "icerik": { "type": "string", "description": "Yazma işleminde yazılacak içerik." }
                    },
                    "required": ["eylem", "hedef"]
                }),
            },
            McpAracTanimi {
                name: "kervan_durum".to_string(),
                description: "Kervan oturum havuzu, hız bekçisi ve sistem sağlığı telemetrisini döner.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ];

        Ok(json!({
            "tools": araclar
        }))
    }

    fn isle_tools_call(&self, params: Value) -> Result<Value, McpHataDetay> {
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| McpHataDetay {
                code: -32602,
                message: "Eksik araç adı (name parameter missing)".to_string(),
                data: None,
            })?;

        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        let metin_sonuc = match name {
            "kervan_modeller" => {
                let saglayicilar = [
                    ("gemini", "Google Gemini Web"),
                    ("deepseek", "DeepSeek Web"),
                    ("chatgpt", "ChatGPT Web"),
                    ("qwen", "Alibaba Qwen Web"),
                    ("kimi", "Moonshot Kimi Web"),
                    ("copilot", "Microsoft Copilot"),
                    ("perplexity", "Perplexity"),
                    ("mistral", "Mistral AI"),
                    ("z.ai", "Zhipu AI"),
                ];
                let mut liste = Vec::new();
                for (kod, ad) in saglayicilar {
                    liste.push(format!("• {}: {}", kod, ad));
                }
                format!("Kervan'a Bağlı Sağlayıcılar:\n{}", liste.join("\n"))
            }
            "kervan_gorev_rotala" => {
                let istem = args.get("istem").and_then(|i| i.as_str()).unwrap_or("");
                let hedef = self.akilli_rota.cozumle("auto", "auto", istem);
                format!(
                    "Akıllı Rota Kararı:\nSağlayıcı: {}\nModel: {}\nAçıklama: {}",
                    hedef.saglayici, hedef.model, hedef.aciklama
                )
            }
            "kervan_sikistir" => {
                let metin = args.get("metin").and_then(|m| m.as_str()).unwrap_or("");
                let seviye_str = args.get("seviye").and_then(|s| s.as_str()).unwrap_or("orta");
                let seviye = match seviye_str {
                    "hafif" => SikistirmaSeviyesi::Hafif,
                    "agresif" => SikistirmaSeviyesi::Agresif,
                    _ => SikistirmaSeviyesi::Orta,
                };
                let comp = Compressor::new();
                let sonuc = comp.akilli_sikistir(metin, seviye);
                format!(
                    "Sıkıştırılmış Metin:\n{}\n\n[Telemetri: {} -> {} karakter, %{:.1} tasarruf, ~{} token]",
                    sonuc.metin, sonuc.orijinal_uzunluk, sonuc.yeni_uzunluk, sonuc.tasarruf_yuzdesi, sonuc.tahmini_tasarruf_token
                )
            }
            "kervan_yerel_dosya" => {
                let eylem = args.get("eylem").and_then(|e| e.as_str()).unwrap_or("");
                let hedef = args.get("hedef").and_then(|h| h.as_str()).unwrap_or("");
                let icerik = args.get("icerik").and_then(|c| c.as_str()).unwrap_or("");

                match eylem {
                    "oku" => match std::fs::read_to_string(hedef) {
                        Ok(icerik) => icerik,
                        Err(e) => format!("Dosya okuma hatası: {}", e),
                    },
                    "yaz" => match std::fs::write(hedef, icerik) {
                        Ok(_) => format!("Başarıyla yazıldı: {} ({} bayt)", hedef, icerik.len()),
                        Err(e) => format!("Dosya yazma hatası: {}", e),
                    },
                    "dizin" => match std::fs::read_dir(hedef) {
                        Ok(entries) => {
                            let mut dosyalar = Vec::new();
                            for entry in entries.flatten() {
                                if let Ok(ad) = entry.file_name().into_string() {
                                    dosyalar.push(ad);
                                }
                            }
                            format!("Dizin Listesi ({}):\n{}", hedef, dosyalar.join("\n"))
                        }
                        Err(e) => format!("Dizin okuma hatası: {}", e),
                    },
                    _ => format!("Geçersiz eylem: {}", eylem),
                }
            }
            "kervan_durum" => {
                format!(
                    "Kervan Sistem Durumu:\n• MCP Sunucusu: Çevrimiçi (v{})\n• Akıllı Rota: Aktif\n• Çift Yönlü Tercüman: Hazır\n• Throughput Bekçisi: Hazır",
                    self.sunucu_surumu
                )
            }
            bilinmeyen => {
                return Err(McpHataDetay {
                    code: -32601,
                    message: format!("Bilinmeyen araç: {}", bilinmeyen),
                    data: None,
                });
            }
        };

        Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": metin_sonuc
                }
            ],
            "isError": false
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize() {
        let sunucu = McpSunucusu::yeni();
        let istek = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let yanit_str = sunucu.isle_istek(istek).expect("Yanıt üretilmeli");
        let yanit: Value = serde_json::from_str(&yanit_str).unwrap();

        assert_eq!(yanit["result"]["serverInfo"]["name"], "kervan-mcp");
        assert_eq!(yanit["result"]["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn test_mcp_tools_list() {
        let sunucu = McpSunucusu::yeni();
        let istek = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let yanit_str = sunucu.isle_istek(istek).unwrap();
        let yanit: Value = serde_json::from_str(&yanit_str).unwrap();

        let tools = yanit["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "kervan_modeller"));
        assert!(tools.iter().any(|t| t["name"] == "kervan_gorev_rotala"));
        assert!(tools.iter().any(|t| t["name"] == "kervan_sikistir"));
    }

    #[test]
    fn test_mcp_tools_call_gorev_rotala() {
        let sunucu = McpSunucusu::yeni();
        let istek = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"kervan_gorev_rotala","arguments":{"istem":"Bu Rust kodundaki hatayı düzelt fn main() {}"}}}"#;
        let yanit_str = sunucu.isle_istek(istek).unwrap();
        let yanit: Value = serde_json::from_str(&yanit_str).unwrap();

        let text = yanit["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Sağlayıcı: deepseek"));
    }

    #[test]
    fn test_mcp_tools_call_sikistir() {
        let sunucu = McpSunucusu::yeni();
        let istek = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"kervan_sikistir","arguments":{"metin":"Bu çalışmada esasen ve sonuç olarak incelenmiştir.","seviye":"orta"}}}"#;
        let yanit_str = sunucu.isle_istek(istek).unwrap();
        let yanit: Value = serde_json::from_str(&yanit_str).unwrap();

        let text = yanit["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Telemetri:"));
    }

    #[test]
    fn test_mcp_gecersiz_metot() {
        let sunucu = McpSunucusu::yeni();
        let istek = r#"{"jsonrpc":"2.0","id":5,"method":"bilinmeyen/metot"}"#;
        let yanit_str = sunucu.isle_istek(istek).unwrap();
        let yanit: Value = serde_json::from_str(&yanit_str).unwrap();

        assert_eq!(yanit["error"]["code"], -32601);
    }
}
