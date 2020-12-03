
use armour_lang::expressions::{self, *};
use armour_lang::interpret::*;
use armour_lang::labels::{*};
use armour_lang::literals::{self, *};
use armour_lang::policies::{self, *};
use armour_lang::types::{*};

use std::collections::{BTreeSet};
use std::path::{PathBuf};
use std::str::FromStr;
use std::sync::Arc;

fn get_policies_path(name: &str) -> PathBuf{ 
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests");
    d.push("policies");
    d.push(name);
    //println!("Loading policy from: {}", d.display());
    d 
}

async fn eval_http_policy<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+'static>(
    function: &str,
    raw_pol: &str,
    args: Vec<Literal<FlatTyp, FlatLiteral>>
) -> Result<Expr<FlatTyp, FlatLiteral>, expressions::Error> {
    let policies = policies::Policies::from_file(
        get_policies_path(raw_pol).to_str().unwrap()
    )?;

    match policies.policy(policies::Protocol::HTTP) {
        Some(policy) => {
            let env : Env<FlatTyp, FlatLiteral> = Env::new(&policy.program);

            let expr : Expr<FlatTyp, FlatLiteral> = expressions::Expr::call(
                function, 
                args.into_iter().map(|x| Expr::LitExpr(x)).collect()
            );
            println!("Expr built");
            expr.print_debug();                    
            println!();
            println!("Evaluating the expression");
            Ok(Expr::evaluate(expr, Arc::new(()), env.clone()).await?)
        },
        _ => Err(expressions::Error::from(format!("interpreter tests, policy loading")))
    }
}

async fn id_pol1<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+'static>(
) ->  Result<Expr<FlatTyp, FlatLiteral>,  expressions::Error> {
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

    let res = eval_http_policy(function, "pol1.policy", args).await?;
    println!("## Expr after eval");            
    res.print_debug();
    Ok(res)
}

async fn log_pol1<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+'static>(
) ->  Result<Expr<FlatTyp, FlatLiteral>,  expressions::Error> {
    let function = "allow_rest_request";

    let mut from_labels: BTreeSet<&str> = vec![ 
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
    ].into_iter().map(&str::to_string).collect();

    let to = literals::ID::new(
        to_hosts, //hosts
        BTreeSet::new(), //ips
        Some(1023), //port
        to_labels.into_iter()
            .map(|x| Label::from_str(x).map_err(|x| expressions::Error::from(x)).unwrap() )
            .collect() 
    );

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

    let res = eval_http_policy(function, "pol2.policy", args).await?;
    println!("## Expr after eval");            
    res.print_debug();
    Ok(res)
}

//FIXME De Bruijn indices not tested
//TODO write some helper fct to test only expr simplification

async fn eval_expr(expr: &str) -> DPExpr {
    let buf = &format!("fn allow_rest_request(from: ID, to: ID, req: HttpRequest, payload: data) -> bool {{ {} }}", expr)[..];
    let policies: DPPolicies = policies::DPPolicies::from_buf(
        buf	
    ).unwrap();
    let policy = policies.policy(policies::Protocol::HTTP).unwrap();
    let env : DPEnv = Env::new(&policy.program);
    let expr : DPExpr = env.get("allow_rest_request").unwrap().at_depth(4).unwrap();
    return DPExpr::evaluate(expr, Arc::new(()), env).await.unwrap(); 
}
mod tests_dplang {
    use super::*;

    #[actix_rt::test]
    async fn test_matches() {
       let label1 = Label::from_str("Service::<<service>>").unwrap();
       let label2 = Label::from_str("Service::Ingress::armour").unwrap();
       assert_eq!(label1.match_with(&label2).unwrap().get_label("service").unwrap().clone(), Label::from_str("Ingress::armour").unwrap());
    } 

    #[actix_rt::test]
    async fn test_eval_req_id() -> Result<(),  expressions::Error> {
        assert_eq!(id_pol1::<FlatTyp, FlatLiteral>().await?, Expr::LitExpr(Literal::bool(false)));
        Ok(())
    }

    #[actix_rt::test]
    async fn test_fold() -> () {
        let res = eval_expr("
        let l = [1, 2];
        let r = fold x in l { acc+x } where acc=0;
        r == 3
        ").await;
        assert_eq!( format!("{}", res), "true");
    }

    #[actix_rt::test]
    async fn test_label() -> () {
        let res = eval_expr("
        Label::concat('Test', 'test') == 'Test::test' 
        ").await;
        assert_eq!( format!("{}", res), "true");
    }

}

mod tests_cplang {
    use super::*;

    #[actix_rt::test]
    async fn test_eval_req_id() -> Result<(),  expressions::Error> {
        assert_eq!(id_pol1::<CPFlatTyp, CPFlatLiteral>().await?, Expr::LitExpr(Literal::bool(false)));
        Ok(())
    }
}