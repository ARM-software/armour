
use armour_lang::expressions::{self, *};
use armour_lang::interpret::*;
use armour_lang::labels::{self, *};
use armour_lang::literals::{self, *};
use armour_lang::policies;
use armour_lang::types::{self, *};

use std::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};

async fn load_policy<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+'static>(
    function: &str,
    from: ID<FlatTyp, FlatLiteral>,
    to: ID<FlatTyp, FlatLiteral>,
    raw_pol: &str,
    args: Vec<Literal<FlatTyp, FlatLiteral>>
) -> Result<Expr<FlatTyp, FlatLiteral>, expressions::Error> {
    println!("Args built");
    let policies = policies::Policies::from_buf(raw_pol)?;
    println!("{} Policies built", policies.len());
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
            Ok(expr.evaluate(env.clone()).await?)
        },
        _ => Err(expressions::Error::from(format!("interpreter tests, policy loading")))
    }
}

async fn id_pol1<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+'static>(
) ->  Result<Expr<FlatTyp, FlatLiteral>,  expressions::Error> {
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

    let raw_pol = "
        fn allow_rest_request(req: HttpRequest) -> bool { 
            let (from, to) = req.from_to(); 
            to.server_ok() && from.has_label('allowed')
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
    ";

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

    let res = load_policy(function, from, to, raw_pol, args).await?;
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

    let raw_pol = "
        external logger @ \"log_sock\" {
        fn log(_) -> ()
        }

        fn allow_rest_request(req: HttpRequest) -> bool {
            logger::log(req);
            true
        }
    ";

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

    let res = load_policy(function, from, to, raw_pol, args).await?;
    println!("## Expr after eval");            
    res.print_debug();
    Ok(res)
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
}

mod tests_cplang {
    use super::*;

    #[actix_rt::test]
    async fn test_eval_req_id() -> Result<(),  expressions::Error> {
        assert_eq!(id_pol1::<CPFlatTyp, CPFlatLiteral>().await?, Expr::LitExpr(Literal::bool(false)));
        Ok(())
    }
}