//! Authenticated TP-Link cloud client: login (+MFA), session, signed WAP requests.

use crate::error::{Error, Result};
use crate::sign::sign;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const APP_NAME: &str = "TP-Link_Tapo_Android";
pub const APP_VER: &str = "3.22.503";
const WAP: &str = "https://n-wap.i.tplinkcloud.com";

/// Persistable session. Serialize to keep the user signed in between runs (contains the cloud token).
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub term_id: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub wap_v2: Option<String>,
    #[serde(default)]
    pub cipc: Option<String>,
    #[serde(default)]
    pub app_server: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        // MFA trusts this terminal id, so a persisted session keeps it stable.
        Session { term_id: Uuid::new_v4().simple().to_string().to_uppercase(), token: None, wap_v2: None, cipc: None, app_server: None }
    }
}

impl Session {
    pub fn logged_in(&self) -> bool {
        self.token.is_some() && self.app_server.is_some()
    }
}

#[derive(Clone)]
struct PendingMfa {
    email: String,
    process_id: String,
}

#[derive(Clone)]
pub struct Cloud {
    http: Client,
    pub session: Session,
    pending: Option<PendingMfa>,
}

impl Cloud {
    /// Build a client that trusts both public CAs and TP-Link's private CA (bundled).
    pub fn new(session: Session) -> Result<Self> {
        let mut http = Client::builder().user_agent("okhttp/4.11.0");
        for cert in rustls_pemfile::certs(&mut &include_bytes!("../tplink_ca.pem")[..]).flatten() {
            if let Ok(c) = reqwest::Certificate::from_der(&cert) {
                http = http.add_root_certificate(c);
            }
        }
        Ok(Cloud { http: http.build()?, session, pending: None })
    }

    pub(crate) fn http(&self) -> &Client { &self.http }
    pub(crate) fn token(&self) -> Result<&str> {
        self.session.token.as_deref().ok_or_else(|| Error::Auth("not logged in".into()))
    }
    pub(crate) fn app_server(&self) -> Result<&str> {
        self.session.app_server.as_deref().ok_or_else(|| Error::Auth("not logged in".into()))
    }
    pub(crate) fn cipc(&self) -> Result<&str> {
        self.session.cipc.as_deref().ok_or_else(|| Error::Auth("not logged in".into()))
    }

    /// Headers the app-server (`/v1/things/...`, `/v2/things`) expects.
    pub(crate) fn app_headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("app-cid", format!("app:{APP_NAME}:{}", self.session.term_id)),
            ("x-app-name", APP_NAME.into()),
            ("x-app-version", APP_VER.into()),
            ("x-term-id", self.session.term_id.clone()),
            ("x-ospf", "Android 14".into()),
            ("x-net-type", "wifi".into()),
            ("x-locale", "en_US".into()),
            ("User-Agent", format!("{APP_NAME}/{APP_VER}")),
        ]
    }

    /// Signed WAP POST. `base` is WAP or the post-login wap_v2; returns the `result` object.
    fn wap(&self, base: &str, path: &str, payload: Value, with_token: bool) -> Result<Value> {
        let mut query = vec![
            ("appName", APP_NAME.to_string()), ("appVer", APP_VER.to_string()),
            ("netType", "wifi".into()), ("termID", self.session.term_id.clone()),
            ("ospf", "Android 14".into()), ("brand", "TPLINK".into()),
            ("locale", "en_US".into()), ("model", "pc".into()),
            ("termName", "Tapo PC Client".into()), ("termMeta", "1".into()),
        ];
        if with_token {
            query.push(("token", self.token()?.to_string()));
        }
        let body = serde_json::to_vec(&payload)?;
        let (md5, auth) = sign(&body, path, &Uuid::new_v4().to_string());
        let r: Value = self
            .http
            .post(format!("{base}{path}"))
            .query(&query)
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("Content-MD5", md5)
            .header("X-Authorization", auth)
            .body(body)
            .send()?
            .json()?;
        if r.get("error_code").map(|c| c.to_string()) != Some("0".into()) {
            return Err(Error::Cloud(format!(
                "{} {}",
                r.get("error_code").unwrap_or(&Value::Null),
                r.get("msg").and_then(Value::as_str).unwrap_or("")
            )));
        }
        Ok(r.get("result").cloned().unwrap_or(Value::Null))
    }

    pub(crate) fn wap_v2(&self, path: &str, payload: Value) -> Result<Value> {
        let base = self.session.wap_v2.clone().ok_or_else(|| Error::Auth("not logged in".into()))?;
        self.wap(&base, path, payload, true)
    }

    /// Start login. On success the session is populated; if MFA is on, returns `Error::MfaRequired`
    /// (the code email has already been triggered) — then call [`complete_mfa`].
    pub fn login(&mut self, email: &str, password: &str) -> Result<()> {
        let creds = json!({
            "appType": APP_NAME, "cloudUserName": email, "cloudPassword": password,
            "terminalUUID": self.session.term_id,
        });
        let r = self.wap(WAP, "/api/v2/account/login", json!({
            "appType": APP_NAME, "cloudUserName": email, "cloudPassword": password,
            "terminalUUID": self.session.term_id, "appVersion": APP_VER, "platform": "Android 14",
            "refreshTokenNeeded": false, "supportBindAccount": false, "terminalMeta": "1",
            "terminalName": "Tapo PC Client",
        }), false)?;

        if r.get("errorCode").map(|c| c.to_string()) == Some("-20677".into()) {
            self.wap(WAP, "/api/v2/account/getEmailVC4TerminalMFA", creds, false)?;
            let process_id = r.get("MFAProcessId").and_then(Value::as_str).unwrap_or_default().to_string();
            let mail = r.get("email").and_then(Value::as_str).unwrap_or(email).to_string();
            self.pending = Some(PendingMfa { email: email.to_string(), process_id });
            return Err(Error::MfaRequired { process_id: r["MFAProcessId"].as_str().unwrap_or_default().to_string(), email: mail });
        }
        self.finish_login(r)
    }

    /// Supply the emailed verification code to finish an MFA login.
    pub fn complete_mfa(&mut self, code: &str) -> Result<()> {
        let p = self.pending.take().ok_or_else(|| Error::Auth("no MFA in progress".into()))?;
        let r = self.wap(WAP, "/api/v2/account/checkMFACodeAndLogin", json!({
            "appType": APP_NAME, "cloudUserName": p.email, "code": code,
            "MFAProcessId": p.process_id, "MFAType": "2",
            "supportBindAccount": false, "terminalBindEnabled": true,
        }), false)?;
        self.finish_login(r)
    }

    fn finish_login(&mut self, r: Value) -> Result<()> {
        let token = r.get("token").and_then(Value::as_str)
            .ok_or_else(|| Error::Auth(r.get("errorMsg").and_then(Value::as_str).unwrap_or("no token").into()))?;
        self.session.token = Some(token.to_string());
        self.session.wap_v2 = r.get("appServerUrlV2").and_then(Value::as_str).map(str::to_string);
        let urls = self.wap_v2("/api/v2/common/getAppServiceUrl",
            json!({"serviceIds": ["cipc.api.cloud", "nbu.iot-app-server.app-v2"]}))?;
        let urls = &urls["serviceUrls"];
        self.session.cipc = urls.get("cipc.api.cloud").and_then(Value::as_str).map(str::to_string);
        self.session.app_server = urls.get("nbu.iot-app-server.app-v2").and_then(Value::as_str).map(str::to_string);
        Ok(())
    }
}
