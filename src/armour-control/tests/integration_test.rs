
use actix_web::{web};

use armour_control::interpret::*;
use armour_control::rest_api::*;
use armour_control::ControlPlaneState;

use armour_lang::expressions::{self, *};
use armour_lang::interpret::{Env, CPEnv, DPEnv};
use armour_lang::labels::{self, *};
use armour_lang::literals::{self, *};
use armour_lang::policies;
use armour_lang::policies_cp::{self, *};
use armour_lang::types::{self, *};
use armour_lang::types_cp::{self, *};

use mongodb::{options::ClientOptions, Client};

use std::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};

type Error = Box<dyn std::error::Error + Send + Sync>;

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

    // start from empty database
    db_con.database("armour").drop(None).await?;
    log::info!("reset armour database");

    Ok(web::Data::new(ControlPlaneState {
        db_endpoint,
        db_con,
    }))
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
            println!();
            println!("Evaluating the expression");
            Ok(expr.evaluate(env.clone()).await?)
        },
        _ => Err(expressions::Error::from(format!("interpreter tests, policy loading")))
    }
}
async fn load_onboarding_policy(
    mock_state: State,
    raw_pol: &str,
    onboarding_data: CPLiteral
) -> Result<CPExpr, expressions::Error> {
    let policy =  policies_cp::OnboardingPolicy::from_buf(raw_pol)?; 
    println!("onboarding policy built");
    let env : CPEnv = Env::new(&policy.program);

    let expr : CPExpr = expressions::Expr::call(
        ONBOARDING_SERVICES, 
        vec![Expr::LitExpr(onboarding_data)]
    );
    println!("Expr built");
    expr.print_debug();                    
    Ok(expr)
}

async fn onboarding_pol1() ->  Result<CPExpr,  expressions::Error> {
    let ob_data = OnboardingData::new(
        Label::from_str("host").map_err(|x| expressions::Error::from(x)).unwrap(),
        Label::from_str("endpoint").map_err(|x| expressions::Error::from(x)).unwrap(),
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
    let raw_pol = "
        fn onboardingPolicy(od: OnboardingData) -> OnboardingResult {
            let ep = od.host();
            let service = od.service();
            if let Some(id) = ControlPlane::onboarded(ep, service) {
                OnboardingResult::Err(\"Endpoint already onboarded\",
                                    id,
                                    compile_ingress(\"allow_rest_request\", id))
            } else {
                let id = ControlPlane::newID(ep, service);
                id.add_label(Label::new(\"SecureService\"));
                id.add_label(Label::login_time(System::getCurrentTime()));
                ControlPlane::onboard(id);
                OnboardingResult::Ok(id, compile_ingress(\"allow_rest_request\", id))            
            }
        }
    ";

    let res = load_onboarding_policy(
        mock_state().await.map_err(|x| expressions::Error::from(x.to_string()))?,
        raw_pol,
        Literal::FlatLiteral(CPFlatLiteral::OnboardingData(Box::new(ob_data)))
    ).await?;
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

    //#[actix_rt::test]
    //async fn test_load_global() -> Result<(),  expressions::Error> {
    //    assert_eq!(global_pol1().await?, Expr::LitExpr(Literal::bool(false)));
    //    Ok(())
    //}
    
    //#[actix_rt::test]
    //async fn test_seval_onboarding() -> Result<(),  expressions::Error> {
    //    let res = onboarding_pol1().await?;
    //    let res_seval = res.sevaluate(&moke_state().await?, env.clone())
    //            .await?;
    //    assert_eq!(res_seval, Expr::LitExpr(Literal::bool(false)));
    //    Ok(())
    //}
    
    //#[actix_rt::test]
    //async fn test_seval() -> Result<(),  expressions::Error> {
    //        println!("Evaluating the expression");
    //        Ok(expr.sevaluate(mock_state, env.clone()).await?)
    //    assert_eq!(onboarding_pol1().await?, Expr::LitExpr(Literal::bool(false)));
    //    Ok(())
    //}
}