use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// A2A Protocol v0.3 Ajan Kartı (Agent Card)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AjanKarti {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub capabilities: AjanYetenekleri,
    pub skills: Vec<AjanBecerisi>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AjanYetenekleri {
    pub streaming: bool,
    #[serde(rename = "pushNotifications")]
    pub push_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AjanBecerisi {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl Default for AjanKarti {
    fn default() -> Self {
        Self {
            name: "Kervan".to_string(),
            description: "Çoklu sağlayıcı ve web oturumu destekli akıllı yapay zeka yönlendiricisi"
                .to_string(),
            url: "http://127.0.0.1:20129/a2a".to_string(),
            version: "0.1.0".to_string(),
            capabilities: AjanYetenekleri {
                streaming: true,
                push_notifications: false,
            },
            skills: vec![
                AjanBecerisi {
                    id: "smart-routing".to_string(),
                    name: "Akıllı Yönlendirme".to_string(),
                    description: "19 kombo stratejisi ile en uygun sağlayıcıyı seçer".to_string(),
                },
                AjanBecerisi {
                    id: "browser-session".to_string(),
                    name: "Tarayıcı Oturumu".to_string(),
                    description: "Web kullanıcı arayüzleri ve doğrulama duvarı köprüsü".to_string(),
                },
            ],
        }
    }
}

/// A2A Görev Yaşam Döngüsü Durumları
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GorevDurumu {
    Iletildi,
    Calisiyor,
    Tamamlandi,
    Basarisiz,
    IptalEdildi,
}

/// A2A Görev Kaydı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aGorev {
    pub id: String,
    pub durum: GorevDurumu,
    pub girdi: String,
    pub cikti: Option<String>,
    pub hata: Option<String>,
}

/// A2A JSON-RPC 2.0 İstek Modeli
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aIstek {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// A2A JSON-RPC 2.0 Yanıt Modeli
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aYanit {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<A2aHata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aHata {
    pub code: i32,
    pub message: String,
}

/// A2A Görev Yöneticisi
pub struct GorevYonetici {
    gorevler: Mutex<HashMap<String, A2aGorev>>,
}

impl Default for GorevYonetici {
    fn default() -> Self {
        Self::yeni()
    }
}

impl GorevYonetici {
    pub fn yeni() -> Self {
        Self {
            gorevler: Mutex::new(HashMap::new()),
        }
    }

    /// Yeni bir görev başlatır
    pub fn gorev_olustur(&self, id: String, girdi: String) -> A2aGorev {
        let gorev = A2aGorev {
            id: id.clone(),
            durum: GorevDurumu::Iletildi,
            girdi,
            cikti: None,
            hata: None,
        };
        let mut kilit = match self.gorevler.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        kilit.insert(id, gorev.clone());
        gorev
    }

    /// Görev durumunu günceller
    pub fn gorev_guncelle(
        &self,
        id: &str,
        yeni_durum: GorevDurumu,
        cikti: Option<String>,
        hata: Option<String>,
    ) -> Option<A2aGorev> {
        let mut kilit = match self.gorevler.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if let Some(g) = kilit.get_mut(id) {
            g.durum = yeni_durum;
            if cikti.is_some() {
                g.cikti = cikti;
            }
            if hata.is_some() {
                g.hata = hata;
            }
            return Some(g.clone());
        }
        None
    }

    /// Görevi getirir
    pub fn gorev_getir(&self, id: &str) -> Option<A2aGorev> {
        let kilit = match self.gorevler.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        kilit.get(id).cloned()
    }

    /// JSON-RPC çağrısını işler
    pub fn cagrıyı_isle(&self, istek: &A2aIstek) -> A2aYanit {
        let req_id = istek.id.clone().unwrap_or(serde_json::Value::Null);

        match istek.method.as_str() {
            "tasks.create" => {
                let girdi = istek
                    .params
                    .as_ref()
                    .and_then(|p| p["input"].as_str())
                    .unwrap_or("")
                    .to_string();
                let gorev_id = format!(
                    "task-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                );
                let gorev = self.gorev_olustur(gorev_id, girdi);
                A2aYanit {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(serde_json::to_value(gorev).unwrap_or(serde_json::Value::Null)),
                    error: None,
                }
            }
            "tasks.get" => {
                let gorev_id = istek
                    .params
                    .as_ref()
                    .and_then(|p| p["id"].as_str())
                    .unwrap_or("");
                if let Some(gorev) = self.gorev_getir(gorev_id) {
                    A2aYanit {
                        jsonrpc: "2.0".to_string(),
                        id: req_id,
                        result: Some(
                            serde_json::to_value(gorev).unwrap_or(serde_json::Value::Null),
                        ),
                        error: None,
                    }
                } else {
                    A2aYanit {
                        jsonrpc: "2.0".to_string(),
                        id: req_id,
                        result: None,
                        error: Some(A2aHata {
                            code: -32602,
                            message: "Görev bulunamadı".to_string(),
                        }),
                    }
                }
            }
            "tasks.cancel" => {
                let gorev_id = istek
                    .params
                    .as_ref()
                    .and_then(|p| p["id"].as_str())
                    .unwrap_or("");
                if let Some(gorev) =
                    self.gorev_guncelle(gorev_id, GorevDurumu::IptalEdildi, None, None)
                {
                    A2aYanit {
                        jsonrpc: "2.0".to_string(),
                        id: req_id,
                        result: Some(
                            serde_json::to_value(gorev).unwrap_or(serde_json::Value::Null),
                        ),
                        error: None,
                    }
                } else {
                    A2aYanit {
                        jsonrpc: "2.0".to_string(),
                        id: req_id,
                        result: None,
                        error: Some(A2aHata {
                            code: -32602,
                            message: "Görev bulunamadı".to_string(),
                        }),
                    }
                }
            }
            _ => A2aYanit {
                jsonrpc: "2.0".to_string(),
                id: req_id,
                result: None,
                error: Some(A2aHata {
                    code: -32601,
                    message: "Bilinmeyen A2A yöntemi".to_string(),
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ajan_karti_varsayilan() {
        let kart = AjanKarti::default();
        assert_eq!(kart.name, "Kervan");
        assert!(kart.capabilities.streaming);
        assert_eq!(kart.skills.len(), 2);
    }

    #[test]
    fn test_gorev_yasam_dongusu() {
        let yonetici = GorevYonetici::yeni();
        let gorev = yonetici.gorev_olustur("g-1".to_string(), "İstem metni".to_string());
        assert_eq!(gorev.durum, GorevDurumu::Iletildi);

        let calisiyor = yonetici
            .gorev_guncelle("g-1", GorevDurumu::Calisiyor, None, None)
            .unwrap();
        assert_eq!(calisiyor.durum, GorevDurumu::Calisiyor);

        let tamam = yonetici
            .gorev_guncelle(
                "g-1",
                GorevDurumu::Tamamlandi,
                Some("Yanıt metni".to_string()),
                None,
            )
            .unwrap();
        assert_eq!(tamam.durum, GorevDurumu::Tamamlandi);
        assert_eq!(tamam.cikti.as_deref(), Some("Yanıt metni"));
    }

    #[test]
    fn test_json_rpc_tasks_akisi() {
        let yonetici = GorevYonetici::yeni();

        // 1. tasks.create
        let req_create = A2aIstek {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tasks.create".to_string(),
            params: Some(serde_json::json!({ "input": "Ajan görevi" })),
        };
        let res_create = yonetici.cagrıyı_isle(&req_create);
        assert!(res_create.error.is_none());
        let res_val = res_create.result.unwrap();
        let gorev_id = res_val["id"].as_str().unwrap();

        // 2. tasks.get
        let req_get = A2aIstek {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "tasks.get".to_string(),
            params: Some(serde_json::json!({ "id": gorev_id })),
        };
        let res_get = yonetici.cagrıyı_isle(&req_get);
        assert!(res_get.error.is_none());
        assert_eq!(res_get.result.unwrap()["durum"], "iletildi");

        // 3. tasks.cancel
        let req_cancel = A2aIstek {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "tasks.cancel".to_string(),
            params: Some(serde_json::json!({ "id": gorev_id })),
        };
        let res_cancel = yonetici.cagrıyı_isle(&req_cancel);
        assert!(res_cancel.error.is_none());
        assert_eq!(res_cancel.result.unwrap()["durum"], "iptal-edildi");
    }
}
