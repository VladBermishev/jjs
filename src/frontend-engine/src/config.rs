use std::{env, sync::Arc};

#[derive(Copy, Clone, Debug)]
pub enum Env {
    Prod,
    Dev,
}

impl Env {
    pub fn is_dev(self) -> bool {
        use Env::*;
        match self {
            Dev => true,
            Prod => false,
        }
    }
}

pub fn derive_key_512(secret: &str) -> Vec<u8> {
    use digest::Digest;
    use rand::{Rng, SeedableRng};
    let secret_hash = {
        let mut hasher = sha3::Sha3_512::new();
        hasher.input(secret.as_bytes());
        let result = &hasher.result()[16..48];
        let mut out = [0 as u8; 32];
        out.copy_from_slice(&result);
        out
    };
    assert_eq!(secret_hash.len(), 32);
    let mut gen = rand_chacha::ChaChaRng::from_seed(secret_hash);
    let key_size = 32;
    let mut out = Vec::with_capacity(key_size);
    for _i in 0..key_size {
        out.push(gen.gen::<u8>());
    }

    out
}

#[derive(Clone, Debug)]
pub struct FrontendConfig {
    pub port: u16,
    pub host: String,
    pub token_mgr: crate::security::TokenMgr,
    pub db_conn: Arc<dyn db::DbConn>,
    pub unix_socket_path: String,
    pub env: Env,
}

impl FrontendConfig {
    pub fn obtain() -> FrontendConfig {
        let port = env::var("JJS_PORT")
            .map_err(|_| ())
            .and_then(|s| s.parse().map_err(|_| ()))
            .unwrap_or(1779);
        let host = env::var("JJS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let environ = env::var("JJS_ENV")
            .map_err(|_| ())
            .and_then(|e| match e.as_str() {
                "dev" | "devel" | "development" => Ok(Env::Dev),
                "prod" | "production" => Ok(Env::Prod),
                _ => Err(()),
            })
            .unwrap_or_else(|_| {
                if cfg!(debug_assertions) {
                    Env::Dev
                } else {
                    Env::Prod
                }
            });
        let secret = env::var("JJS_SECRET_KEY").unwrap_or_else(|_| {
            if let Env::Dev = environ {
                String::from("DEVEL_HARDCODED_TOKEN")
            } else {
                panic!("Error: running in production mode, but JJS_SECRET_KEY not specified");
            }
        });
        let secret = derive_key_512(&secret);
        let db_conn: Arc<dyn db::DbConn> = db::connect_env()
            .expect("initialize db connection failed")
            .into();
        let unix_socket_path =
            env::var("JJS_UNIX_SOCKET_PATH").unwrap_or_else(|_| "/tmp/jjs-auth-sock".to_string());

        let token_mgr = crate::security::TokenMgr::new(db_conn.clone(), secret.clone().into());

        FrontendConfig {
            port,
            host,
            db_conn,
            unix_socket_path,
            env: environ,
            token_mgr,
        }
    }
}
