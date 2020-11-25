
use actix_web::{web};

use armour_api::control::*;

use armour_control::State;
use armour_control::interpret::*;
use armour_control::rest_api::*;
use armour_control::specialize::*;
use armour_control::ControlPlaneState;

use armour_lang::expressions::{self, *};
use armour_lang::interpret::{Env, CPEnv, DPEnv, TExprInterpreter};
use armour_lang::labels::{self, *};
use armour_lang::literals::{self, *};
use armour_lang::policies::{self, *};

use bson::doc;
use mongodb::{options::ClientOptions, Client};

use std::path::{PathBuf};
use std::sync::Arc;
use std::str::FromStr;
use std::collections::{BTreeSet};
use std::iter::Iterator;

type Error = Box<dyn std::error::Error + Send + Sync>;
//clear && RUST_MIN_STACK=8388608 cargo test -j 20 -- --nocapture test_seval_onboarding
//rsync -avz src vagrant@localhost:~/ -e "ssh -p 2222 -i /home/marmotte/armour/examples/.vagrant/machines/default/virtualbox/private_key" --exclude=target/
//rsync -avz /home/marmotte/armour/src/armour-proxy/src/policy.rs vagrant@localhost:~/src/armour-proxy/src/policy.rs -e "ssh -p 2222 -i /home/marmotte/armour/examples/.vagrant/machines/default/virtualbox/private_key" 


fn get_policies_path(name: &str) -> PathBuf{ 
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests");
    d.push("policies");
    d.push(name);
    //println!("Loading policy from: {}", d.display());
    d 
}

async fn mock_state () -> Result<State, Error> {
    let mongo_url = "mongodb://localhost:27017" ;

    // connect to MongoDB
    let mut db_endpoint = ClientOptions::parse(mongo_url).await.map_err(|e| {
        log::warn!("failed to get db_endpoint");
        e
    })?;
    db_endpoint.app_name = Some("armour".to_string());
    let db_con = Client::with_options(db_endpoint.clone()).map_err(|e| {
        log::info!("Failed to connect to Mongo. Start MongoDB");
        e
    })?;


    Ok(web::Data::new(ControlPlaneState {
        db_endpoint,
        db_con,
    }))
}

async fn register_policy(
    state: &State,
    raw_pol : &str,
) -> Result<bool, expressions::Error> {
    let request = CPPolicyUpdateRequest {
        label: global_policy_label(),
        policy: policies::Policies::from_file(raw_pol)?,
        labels: LabelMap::default(),
        selector: None 
    };
    let label = &request.label.clone();
    //println!(r#"updating policy for label "{}""#, label);


    if let bson::Bson::Document(document) = to_bson(&request).map_err(|x|expressions::Error::from(format!("{:?}", x)))? {
        // update policy in database
        let col = collection(&state, POLICIES_COL);
        let filter = doc! { "label" : to_bson(label).map_err(|x|expressions::Error::from(format!("{:?}", x)))? };
        col.delete_many(filter, None)
            .await
            .map_err(|_| expressions::Error::from(format!("error removing old policies")))?;
        col.insert_one(document, None)
            .await
            .map_err(|_| expressions::Error::from(format!("error inserting new policy")))?;
        Ok(true)
    } else {
        println!("error converting the BSON object into a MongoDB document");
        Ok(false)
    }
}


async fn register_onboarding_policy(
    state: &State,
    raw_pol : &str,
) -> Result<bool, expressions::Error> {
    let request = OnboardingUpdateRequest {
        label: onboarding_policy_label(),
        policy: policies::OnboardingPolicy::from_file(raw_pol)?,
        labels: LabelMap::default()
    }.pack();
    let label = &request.label.clone();
    //println!(r#"updating policy for label "{}""#, label);

    if let bson::Bson::Document(document) = to_bson(&request).map_err(|x|expressions::Error::from(format!("{:?}", x)))? {
        // update policy in database
        let col = collection(&state, POLICIES_COL);
        let filter = doc! { "label" : to_bson(label).map_err(|x|expressions::Error::from(format!("{:?}", x)))? };
        col.delete_many(filter, None)
            .await
            .map_err(|_| expressions::Error::from(format!("error removing old policies")))?;
        col.insert_one(document, None)
            .await
            .map_err(|_| expressions::Error::from(format!("error inserting new policy")))?;
        Ok(true)
    } else {
        println!("error converting the BSON object into a MongoDB document");
        Ok(false)
    }
}

async fn load_global_policy(
    function: &str,
    raw_pol: &str,
    args: Vec<CPLiteral>
) -> Result<(CPExpr, CPEnv), expressions::Error> {
    //println!("Args built");
    let policies: GlobalPolicies = policies::GlobalPolicies::from_file(raw_pol)?;
    //println!("{} Policies built", policies.len());
    match policies.policy(policies::Protocol::HTTP) {
        Some(policy) => {
            let env : CPEnv = Env::new(&policy.program);

            let expr : CPExpr = expressions::Expr::call(
                function, 
                args.into_iter().map(|x| Expr::LitExpr(x)).collect()
            );
            //println!("Expr built");
            expr.print_debug();                    
            Ok((expr, env))
        },
        _ => Err(expressions::Error::from(format!("interpreter tests, policy loading")))
    }
}
async fn load_onboarding_policy(
    raw_pol: &str,
    onboarding_data: CPLiteral
) -> Result<(CPExpr, CPEnv), expressions::Error> {
    let policy =  policies::OnboardingPolicy::from_file(raw_pol)?; 
    //println!("onboarding policy built");
    let env : CPEnv = Env::new(&policy.program);

    let expr : CPExpr = expressions::Expr::call(
        ONBOARDING_SERVICES, 
        vec![Expr::LitExpr(onboarding_data)]
    );
    //println!("Expr built");
    expr.print_debug();                    
    Ok((expr, env))
}

async fn onboarding_pol1() ->  Result<(CPExpr, CPEnv),  expressions::Error> {
    let proposed_labels: BTreeSet<&str> = vec![ 
        "proposed1",
    ].into_iter().collect(); 

    let ob_data = OnboardingData::new(
        Label::from_str("host").map_err(|x| expressions::Error::from(x)).unwrap(),
        Label::from_str("service").map_err(|x| expressions::Error::from(x)).unwrap(),
        Some(80),
        proposed_labels.into_iter()
        .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
        .collect(),
        BTreeSet::default()
    );

    let (pol, env) = load_onboarding_policy(
        get_policies_path("onboard1.policy").to_str().unwrap(),
        Literal::FlatLiteral(CPFlatLiteral::OnboardingData(Box::new(ob_data)))
    ).await?;
    //println!("## Policy expr built");            
    pol.print_debug();
    Ok((pol, env))
}

fn get_from_to() -> Result<(DPID, DPID), expressions::Error> {
    let from_labels: BTreeSet<&str> = vec![ 
        "allowed",
    ].into_iter().collect(); 

    let from = literals::ID::new(
        BTreeSet::new(), //hosts
        BTreeSet::new(), //ips
        Some(80), //port
        from_labels.into_iter()
            .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
            .collect() 
    );
    
    let to_labels: BTreeSet<&str> = vec![ 
    ].into_iter().collect();
    let to_hosts: BTreeSet<String> = vec![ 
        "server"
    ].into_iter().map(&str::to_string).collect();

    let to = literals::ID::new(
        to_hosts, //hosts
        BTreeSet::new(), //ips
        Some(80), //port
        to_labels.into_iter()
            .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
            .collect() 
    );
    Ok((from, to))
}

async fn global_pol1() ->  Result<(CPExpr, CPEnv),  expressions::Error> {
    let function = "allow_rest_request";

    let from_labels: BTreeSet<&str> = vec![ 
        "allowed",
    ].into_iter().collect(); 

    let from = literals::ID::new(
        BTreeSet::new(), //hosts
        BTreeSet::new(), //ips
        Some(1023), //port
        from_labels.into_iter()
            .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
            .collect() 
    );
    
    let to_labels: BTreeSet<&str> = vec![ 
    ].into_iter().collect();
    let to_hosts: BTreeSet<String> = vec![ 
        "server"
    ].into_iter().map(&str::to_string).collect();

    let to = literals::ID::new(
        to_hosts, //hosts
        BTreeSet::new(), //ips
        Some(1023), //port
        to_labels.into_iter()
            .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
            .collect() 
    );
// &&
//                match req.path() {
//                    \"/private\" => {
//                        from.has_label(Label::new(\"SecureService\")) && payload.len() == 0
//                    }
//                    _ => {
//                        payload.len() == 0
//                    }
//                }
    let args = vec![
        Literal::http_request(Box::new(HttpRequest::new(
            "method",
            "HTTP_20",
            "path",
            "query", 
            Vec::new(),
            literals::Connection::from((&from, &to, 1)),
        ))),
        //Literal::data(Vec::new()) 
    ];

    let (res, env) = load_global_policy(function, get_policies_path("global1.policy").to_str().unwrap(), args).await?;
    //println!("## Expr after eval");            
    res.print_debug();
    Ok((res, env))
}
mod tests_control {
    use super::*;
    
    #[actix_rt::test]
    async fn test_load_onboarding() -> Result<(),  expressions::Error> {
        onboarding_pol1().await?; 
        Ok(())
    }

    #[actix_rt::test]
    async fn test_load_global() -> Result<(),  expressions::Error> {
        global_pol1().await?;
        Ok(())
    }
    
    #[actix_rt::test]
    async fn test_helper_compile_ingress() -> Result<(),  expressions::Error> {
        let state = mock_state().await.map_err(|x|expressions::Error::from(format!("{:?}", x)))?;
        state.db_con.database("armour").drop(None).await.map_err(|x|expressions::Error::from(format!("{:?}", x)))?;
        register_policy(&state, get_policies_path("global1.policy").to_str().unwrap()).await?;
        
        if let Ok(Some(doc)) = collection(&state.clone(), POLICIES_COL)
            .find_one(Some(doc! {"label" : to_bson(&global_policy_label()).unwrap()}), None)
            .await
        {
            assert!(true)
        }else {
            assert!(false);
        }

        let labels: BTreeSet<&str> = vec![ 
            "allowed",
        ].into_iter().collect(); 

        let id = literals::ID::new(
            BTreeSet::new(), //hosts
            BTreeSet::new(), //ips
            Some(80), //port
            labels.into_iter()
                .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
                .collect() 
        );

        match helper_compile_ingress(
            Arc::new(state),
            &"allow_rest_request".to_string(),
            &id
        ).await? {
            Literal::FlatLiteral(CPFlatLiteral::Policy(_)) => assert!(true),
            _ => assert!(false)
        }
        Ok(())
    }

    #[actix_rt::test]
    async fn test_seval_onboarding() -> Result<(),  expressions::Error> {
        let state = mock_state().await.map_err(|x|expressions::Error::from(format!("{:?}", x)))?;
        state.db_con.database("armour").drop(None).await.unwrap();
        register_policy(&state, get_policies_path("global1.policy").to_str().unwrap()).await?;

        if let Ok(Some(doc)) = collection(&state.clone(), POLICIES_COL)
            .find_one(Some(doc! {"label" : to_bson(&global_policy_label()).unwrap()}), None)
            .await
        {
            assert!(true)
        }else {
            assert!(false);
        }

        let (expr, env) = onboarding_pol1().await?;
        let res_seval = CPExprWrapper::evaluate(expr, Arc::new(state), env.clone()).await?;
        
        match res_seval {
            Expr::LitExpr(Literal::FlatLiteral(CPFlatLiteral::OnboardingResult(_))) =>{
                //println!("OnboardingResult\n{:#?}", r );
                assert!(true)
            },
            _ => assert!(false)
        }
        Ok(())
    }
    
    //#[actix_rt::test]
    //async fn test_onboard_pol1_obd1() -> Result<(),  actix_web::Error> {
    //    let state = mock_state().await.unwrap();
    //    state.db_con.database("armour").drop(None).await;
    //    register_policy(&state, raw_pol1()).await.unwrap();
    //    register_onboarding_policy(&state, raw_onboard1()).await.unwrap();

    //    let request = OnboardServiceRequest{
    //        service: labels::Label::from_str("Service21").unwrap(),
    //        host: labels::Label::from_str("Host42").unwrap()
    //    };

    //    Ok(match service::helper_on_board(&state, request).await? {
    //        Ok(req) => assert_eq!(1,1),
    //        Err(res) => panic!(res)
    //    })
    //}
    
//    #[actix_rt::test]
//    async fn test_onboard_pol2_obd1() -> Result<(),  actix_web::Error> {
//        let state = mock_state().await.unwrap();
//        state.db_con.database("armour").drop(None).await;
//        register_policy(&state, raw_pol2()).await.unwrap();
//        register_onboarding_policy(&state, raw_onboard1()).await.unwrap();
//
//        let request = OnboardServiceRequest{
//            service: labels::Label::from_str("Service21").unwrap(),
//            host: labels::Label::from_str("Host42").unwrap()
//        };
//
//        Ok(match service::helper_on_board(&state, request).await? {
//            Ok(req) => assert_eq!(req.policy.policy(Protocol::HTTP).unwrap().program.to_string(), 
//                                  "fn allow_rest_request(req: HttpRequest, payload: data) -> bool {
//  req.method() == \"GET\" && true
//}
//"
//                                ),
//            Err(res) => panic!(res)
//        })
//    }

    #[actix_rt::test]
    async fn test_onboard() -> Result<(),  actix_web::Error> {
        let state = mock_state().await.unwrap();
        state.db_con.database("armour").drop(None).await.unwrap();
        register_policy(&state, get_policies_path("global1.policy").to_str().unwrap()).await.unwrap();
        register_onboarding_policy(&state, get_policies_path("onboard1.policy").to_str().unwrap()).await.unwrap();

        let request = OnboardServiceRequest{
            service: labels::Label::from_str("Service21::ingress").unwrap(),
            host: labels::Label::from_str("Host42").unwrap(),
            tmp_dpid: Some(literals::DPID::new(
                BTreeSet::default(),
                BTreeSet::default(),
                Some(80),
                BTreeSet::default()
            ))
        };

        Ok(match service::helper_on_board(&state, request).await? {
            Ok((service_id, ingress_req, egress_req)) =>{
                let fn_ingress = ingress_req.policy.policy(Protocol::HTTP).unwrap().fn_policies.0.get("allow_rest_request");
                let fn_egress = egress_req.policy.policy(Protocol::HTTP).unwrap().fn_policies.0.get("allow_rest_response");
                let merged_policy = ingress_req.policy.merge(&egress_req.policy);
                let fn_egress_m = merged_policy.policy(Protocol::HTTP).unwrap().fn_policies.0.get("allow_rest_response");
                let fn_ingress_m = merged_policy.policy(Protocol::HTTP).unwrap().fn_policies.0.get("allow_rest_request");
                assert_eq!(fn_egress, Some(&FnPolicy::Args(2)));
                assert_eq!(fn_ingress, Some(&FnPolicy::Args(2)));
                assert_eq!(fn_egress_m, Some(&FnPolicy::Args(2)));
                assert_eq!(fn_ingress_m, Some(&FnPolicy::Args(2)));
                //println!("Updating policy for label {}\n{}", ingress_req.label, ingress_req.policy)
            },
            Err(res) => panic!(res)
        })
    }
    
    #[actix_rt::test]
    async fn test_eval_specialize() -> Result<(),  actix_web::Error> {
        let state = mock_state().await.unwrap();
        state.db_con.database("armour").drop(None).await.unwrap();
        register_policy(&state, get_policies_path("global-id.policy").to_str().unwrap()).await.unwrap();
        register_onboarding_policy(&state, get_policies_path("onboard1.policy").to_str().unwrap()).await.unwrap();

        let request = OnboardServiceRequest{
            service: labels::Label::from_str("Service21::ingress").unwrap(),
            host: labels::Label::from_str("Host42").unwrap(),
            tmp_dpid: Some(literals::DPID::new(
                BTreeSet::default(),
                BTreeSet::default(),
                Some(80),
                BTreeSet::default()
            ))
        };

        Ok(match service::helper_on_board(&state, request).await? {
            Ok((service_id, ingress_req, egress_req)) =>{
                //println!("Updating policy for label {}\n{:#?}", ingress_req.label, ingress_req.policy);
                let (from, to) = get_from_to().unwrap();
                let req =  literals::HttpRequest::new("GET", "1", "/", "", Vec::new(), Connection::new(&from, &to, 10));
                let args : Vec<DPExpr> = vec![
                    Expr::LitExpr(DPLiteral::http_request(Box::new(req))),
                    Expr::LitExpr(DPLiteral::data(Vec::new())),
                ];
                println!("{:?}", ingress_req.policy.policy(policies::Protocol::HTTP).unwrap());
                let env : DPEnv = Env::new(&ingress_req.policy.policy(policies::Protocol::HTTP).unwrap().program);
                let result = Expr::evaluate(
                    expressions::Expr::call("allow_rest_request", args),
                    Arc::new(()),
                    env.clone()
                ).await;
                println!{"{:#?}", result};
            },
            Err(res) => panic!(res)
        })
    }


    //TODO write some helper fct to test only expr simplification

    async fn simplify_expr(s_expr: &str) -> (bool, CPExpr) {
        let buf = &format!("fn allow_rest_request(from: ID, to: ID, req: HttpRequest, payload: data) -> bool {{ {} }}", s_expr)[..];
		let policies: GlobalPolicies = policies::GlobalPolicies::from_buf(
            buf	
		).unwrap();
		let policy = policies.policy(policies::Protocol::HTTP).unwrap();
		let env : CPEnv = Env::new(&policy.program);

		let args = vec![
			Literal::http_request(Box::new(HttpRequest::new(
				"method",
				"HTTP_20",
				"path",
				"query", 
				Vec::new(),
				literals::Connection::from((&literals::CPID::default(), &literals::CPID::default(), 1)),
			))),
			//Literal::data(Vec::new()) 
		];	
		let expr : CPExpr = expressions::Expr::call(
			"allow_rest_request", 
			args.into_iter().map(|x| Expr::LitExpr(x)).collect()
		);
        return expr.pevaluate(Arc::new(mock_state().await.unwrap()), env, true).await.unwrap(); 
    }

    #[actix_rt::test]
    async fn let_elimination() {
        let (flag, res) = simplify_expr("let a = 1; true").await; 
        //assert!(flag); //FIXME flag should be true ?
        assert_eq!( format!("{}", res), "true"); //FIXME De Bruijn indices not tested

    }
    #[actix_rt::test]
    async fn if_elimination_1() {
        let (flag, res) = simplify_expr(
			"if 1 == 2 {
                false   
            } else {
                true
            } 
			"
        ).await; 
        //assert!(flag); //FIXME flag should be true ?
        assert_eq!( format!("{}", res), "true"); //FIXME De Bruijn indices not tested
    }
    #[actix_rt::test]
    async fn if_elimination_2() {
        let (flag, res) = simplify_expr(
			"if 2 == 2 {
                false   
            } else {
                true
            } 
			"
        ).await; 
        //assert!(flag); //FIXME flag should be true ?
        assert_eq!( format!("{}", res), "false"); //FIXME De Bruijn indices not tested
    }
    #[actix_rt::test]
    async fn if_no_elimination() {//FIXME do not pass yet we have a \payload. ... why ? regression 
        let (flag, res) = simplify_expr(
			"if req.path() == \"\" { false } else { true }"
        ).await; 
        //assert!(flag); //FIXME flag should be true ?
        assert_eq!( 
            format!("{}", res), 
			"if req.path() == \"\" { false } else { true }"
        ); //FIXME De Bruijn indices not tested
    }
    #[actix_rt::test]
    async fn ifsomematch_elimination_1() {
        let (flag, res) = simplify_expr(
            "if let Some(x) = None { false } else { true }"
        ).await; 
        //assert!(flag); //FIXME flag should be true ?
        assert_eq!( format!("{}", res), "true"); //FIXME De Bruijn indices not tested
    }

    #[actix_rt::test]
    async fn ifsomematch_elimination_2() {//TODO pass yet, regression
        let (flag, res) = simplify_expr(
			"if let Some(x) = Some(1) {
                false 
            } else {
                true
            } 
			"
        ).await; 
        //assert!(flag); //FIXME flag should be true ?
        assert_eq!( format!("{}", res), "false"); //FIXME De Bruijn indices not tested
    }
}