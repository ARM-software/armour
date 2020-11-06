
use actix_web::{web};

use armour_api::control::*;

use armour_control::interpret::*;
use armour_control::rest_api::*;
use armour_control::ControlPlaneState;

use armour_lang::expressions::{self, *};
use armour_lang::interpret::{Env, CPEnv, DPEnv};
use armour_lang::labels::{self, *};
use armour_lang::literals::{self, *};
use armour_lang::policies::{self, *};
use armour_lang::types::{self, *};

use bson::doc;
use mongodb::{options::ClientOptions, Client};


use std::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};
use std::iter::Iterator;
use futures::{future::BoxFuture, Stream};

type Error = Box<dyn std::error::Error + Send + Sync>;
//clear && RUST_MIN_STACK=8388608 cargo test -j 20 -- --nocapture test_seval_onboarding
//rsync -avz src vagrant@localhost:~/ -e "ssh -p 2222 -i /home/marmotte/armour/examples/.vagrant/machines/default/virtualbox/private_key" --exclude=target/
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
        policy: policies::Policies::from_buf(raw_pol)?,
        labels: LabelMap::default(),
        selector: None 
    };
    let label = &request.label.clone();
    println!(r#"updating policy for label "{}""#, label);

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
        policy: policies::OnboardingPolicy::from_buf(raw_pol)?,
        labels: LabelMap::default()
    }.pack();
    let label = &request.label.clone();
    println!(r#"updating policy for label "{}""#, label);

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
    from: CPID,
    to: CPID,
    raw_pol: &str,
    args: Vec<CPLiteral>
) -> Result<CPExpr, expressions::Error> {
    println!("Args built");
    let policies = policies::Policies::from_buf(raw_pol)?;
    println!("{} Policies built", policies.len());
    match policies.policy(policies::Protocol::HTTP) {
        Some(policy) => {
            let env : CPEnv = Env::new(&policy.program);

            let expr : CPExpr = expressions::Expr::call(
                function, 
                args.into_iter().map(|x| Expr::LitExpr(x)).collect()
            );
            println!("Expr built");
            expr.print_debug();                    
            Ok(expr)
        },
        _ => Err(expressions::Error::from(format!("interpreter tests, policy loading")))
    }
}
async fn load_onboarding_policy(
    mock_state: State,
    raw_pol: &str,
    onboarding_data: CPLiteral
) -> Result<(CPExpr, CPEnv), expressions::Error> {
    let policy =  policies::OnboardingPolicy::from_buf(raw_pol)?; 
    println!("onboarding policy built");
    let env : CPEnv = Env::new(&policy.program);

    let expr : CPExpr = expressions::Expr::call(
        ONBOARDING_SERVICES, 
        vec![Expr::LitExpr(onboarding_data)]
    );
    println!("Expr built");
    expr.print_debug();                    
    Ok((expr, env))
}

fn raw_onboard1() -> &'static str {
"
    fn onboarding_policy(obd: OnboardingData) -> OnboardingResult {
        let ep = obd.host();
        let service = obd.service();

        if let Some(id) = ControlPlane::onboarded(ep, service) {
            OnboardingResult::Err(\"Endpoint already onboarded\",
                                id,
                                compile_ingress(\"allow_rest_request\", id),
                                compile_egress(\"allow_rest_request\", id)
                            )
        } else {
            let id = ControlPlane::newID(obd);
            let id = id.add_label(Label::new(\"SecureService\"));
            let id = id.add_label(Label::login_time(System::getCurrentTime()));
            let pol = (compile_ingress(\"allow_rest_request\", id),compile_egress(\"allow_rest_request\", id));
            if ControlPlane::onboard(id) {
                OnboardingResult::Ok(id, pol.0, pol.1)            
            } else {
                OnboardingResult::Err(\"Onboard failure\",
                                id,
                                pol.0, pol.1)


            }
        }
    }
"
}

async fn onboarding_pol1() ->  Result<(CPExpr, CPEnv),  expressions::Error> {
    let ob_data = OnboardingData::new(
        Label::from_str("host").map_err(|x| expressions::Error::from(x)).unwrap(),
        Label::from_str("service").map_err(|x| expressions::Error::from(x)).unwrap(),
        Some(80),
    );

    //let raw_pol = "
    //    fn onboardingPolicy(od: OnboardingData) -> OnboardingResult {
    //        let ep = od.endpoint();
    //        match od.declaredDomain() {
    //            \"SecureServices\" => {
    //                if let Some(token) = verifyCredentials(ob.credentials(),
    //                                                    MyPolicyConstants::SecureCredentials) {
    //                    if let Some(id) = ControlPlane::onboarded(ep) {
    //                        OnboardingResultFail(\"Endpoint already onboarded\",
    //                                            id,
    //                                            getPolicy(id))
    //                    } else {
    //                        let id = ControlPlane::newID(ep);
    //                        ControlPlane::onboard(id);
    //                        id.set_label(MyPolicy::SecureService);
    //                        id.set_label(Token : token);
    //                        id.set_label(LoginTime : System.getCurrentTime());
    //                        OnboardingResultOk(id, getPolicy(id))
    //                    }
    //                }
    //                else {
    //                    OnboardingResultFail(\"Failed to authenticate\", Armour::Policies::DenyAll)
    //                }
    //            }
    //            _ => {
    //                if let Some(id) = ControlPlane::onboarded(ep) {
    //                    OnboardingResultFail(\"Endpoint already onboarded\",
    //                                        id,
    //                                        getPolicy(id))
    //                } else {
    //                    id = ControlPlane::newID(ep);
    //                    ControlPlane::onboard(id);
    //                    id.set_label(MyPolicy::UntrustedService);
    //                    id.set_label(LoginTime : System.getCurrentTime());
    //                    OnboardingResultOk(id, getPolicy(id))
    //                }
    //            }
    //        }
    //    }

    //    fn getPolicy(id: ControlPlane::Id) -> Policy {
    //        let ing = compile_ingress(allow_rest_request, to = id); // These are Armour primitives
    //        let egr = compile_egress(allow_rest_request, from = id);
    //        Policy(ing, egr)
    //    }
    //        ControlPlane::onboard(id);
    //        id.set_label(\"MyPolicy::SecureService\");
    //        id.set_label(LoginTime(getCurrentTime()));
    //";
    let raw_pol = raw_onboard1();

    let (pol, env) = load_onboarding_policy(
        mock_state().await.map_err(|x| expressions::Error::from(x.to_string()))?,
        raw_pol,
        Literal::FlatLiteral(CPFlatLiteral::OnboardingData(Box::new(ob_data)))
    ).await?;
    println!("## Policy expr built");            
    pol.print_debug();
    Ok((pol, env))
}

fn raw_pol1() -> &'static str {
    "
        fn allow_rest_request(from: ID, to: ID, req: HttpRequest, payload: data) -> bool {
            match_to_from(from, to, req) &&
            server_ok(to) &&
                req.method() == \"GET\"
        }
    
        fn server_ok(id: ID) -> bool {
            \"server\" in id.hosts() &&
                if let Some(port) = id.port() {
                    port == 80
                } else {
                    // default is port 80
                    true
                }
        }

        fn match_to_from(from: ID, to: ID, req: HttpRequest) -> bool {
            let (rfrom, rto) = req.from_to();
            true
            //rfrom in from.hosts() && rto in to.hosts(), hosts should be ID not string ??
        }
    "
}

fn raw_pol2() -> &'static str {
    "
        fn allow_rest_request(from: ID, to: ID, req: HttpRequest, payload: data) -> bool {
            req.method() == \"GET\" && to.has_label(Label::new(\"SecureService\")) 
        }
    "
}

async fn global_pol1() ->  Result<CPExpr,  expressions::Error> {
    let function = "allow_rest_request";

    let mut from_labels: BTreeSet<&str> = vec![ 
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
    let raw_pol = raw_pol1();

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

    let res = load_global_policy(function, from, to, raw_pol, args).await?;
    println!("## Expr after eval");            
    res.print_debug();
    Ok(res)
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
        state.db_con.database("armour").drop(None).await;
        register_policy(&state, raw_pol1()).await?;
        
        if let Ok(Some(doc)) = collection(&state.clone(), POLICIES_COL)
            .find_one(Some(doc! {"label" : to_bson(&global_policy_label()).unwrap()}), None)
            .await
        {
            assert!(true)
        }else {
            assert!(false);
        }

        let mut labels: BTreeSet<&str> = vec![ 
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
            state,
            &"allow_rest_request".to_string(),
            &id
        ).await? {
            Literal::FlatLiteral(CPFlatLiteral::Policy(_)) => assert!(true),
            l => assert!(false)
        }
        Ok(())
    }

    #[actix_rt::test]
    async fn test_seval_onboarding() -> Result<(),  expressions::Error> {
        let state = mock_state().await.map_err(|x|expressions::Error::from(format!("{:?}", x)))?;
        state.db_con.database("armour").drop(None).await;
        register_policy(&state, raw_pol1()).await?;

        if let Ok(Some(doc)) = collection(&state.clone(), POLICIES_COL)
            .find_one(Some(doc! {"label" : to_bson(&global_policy_label()).unwrap()}), None)
            .await
        {
            assert!(true)
        }else {
            assert!(false);
        }

        let (expr, env) = onboarding_pol1().await?;
        let res_seval = expr.sevaluate(&state, env.clone()).await?;
        
        match res_seval {
            Expr::LitExpr(Literal::FlatLiteral(r @ CPFlatLiteral::OnboardingResult(_))) =>{
                println!("OnboardingResult\n{:#?}", r );
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
        state.db_con.database("armour").drop(None).await;
        register_policy(&state, raw_pol1()).await.unwrap();
        register_onboarding_policy(&state, raw_onboard1()).await.unwrap();

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
            Ok((service_id, ingress_req, egress_req)) => println!("Updating policy for label {}\n{}", ingress_req.label, ingress_req.policy),
            Err(res) => panic!(res)
        })
    }
}