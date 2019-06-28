use frontend_api::{SubmissionState, SubmissionsListParams, SubmissionsSetInfoParams};
use serde_json::Value;
use std::process::exit;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opt {
    /// Action: view, remove or rejudge
    action: String,
    _filter: String,
}

pub fn exec(opt: Opt, params: &super::CommonParams) -> Value {
    // at first, load submissions from DB
    // TODO optimizations
    let subm_list_query = SubmissionsListParams {
        limit: u32::max_value(),
    };
    let submissions = params
        .client
        .submissions_list(&subm_list_query)
        .unwrap()
        .expect("request rejected");
    match opt.action.as_str() {
        "view" => serde_json::to_value(&submissions).unwrap(),
        "remove" => {
            let mut result = vec![];
            for sbm in &submissions {
                let id = sbm.id;
                //println!("deleting submission {}", id);
                result.push(id);
                let query = frontend_api::SubmissionsSetInfoParams {
                    delete: true,
                    rejudge: false,
                    status: None,
                    state: None,
                    id,
                };
                params
                    .client
                    .submissions_modify(&query)
                    .unwrap()
                    .unwrap_or_else(|_| panic!("request rejected when deleting submission {}", id));
            }
            serde_json::to_value(result).unwrap()
        }
        "rejudge" => {
            let mut result = vec![];
            for sbm in &submissions {
                let id = sbm.id;
                result.push(id);
                let query = SubmissionsSetInfoParams {
                    delete: false,
                    rejudge: false,
                    id,
                    status: None,
                    state: Some(SubmissionState::Queue),
                };
                params
                    .client
                    .submissions_modify(&query)
                    .unwrap()
                    .unwrap_or_else(|_| {
                        panic!(
                            "request rejected when marking submission {} for rejudge",
                            id
                        )
                    });
            }
            serde_json::to_value(result).unwrap()
        }

        _ => {
            eprintln!("unknown submissions command: {}", opt.action);
            exit(1);
        }
    }
}
